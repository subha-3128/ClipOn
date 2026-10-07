use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::analysis_cache::AnalysisCache;
use crate::credentials;
use crate::http_client;
use crate::media::face_tracker::{FaceTracker, FaceTrackerResult};
use crate::media::ffmpeg::resolve_binary;
use crate::media::filters::TrackingKeyframe;
use crate::models::NormalizedTranscript;

// =========================================================================
// 1. DATA MODELS & ABSTRACTIONS
// =========================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveSpeakerSegment {
    pub start: f64,
    pub end: f64,
    pub person_id: usize, // 1-indexed (e.g. 1 for Person 1, 2 for Person 2)
    pub confidence: f64,  // 0.0 to 1.0
    #[serde(default)]
    pub speaker_label: Option<String>, // e.g. "S1", "S2"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSpeakerTimeline {
    pub segments: Vec<ActiveSpeakerSegment>,
    pub provider: String, // "nvidia_api" or "local_vision_fusion"
    pub source_duration: f64,
    pub speaker_person_mapping: HashMap<String, usize>,
    pub confidence: f64,
    #[serde(default)]
    pub fallback_reason: Option<String>,
}

impl Default for ActiveSpeakerTimeline {
    fn default() -> Self {
        Self {
            segments: Vec::new(),
            provider: "local_vision_fusion".to_string(),
            source_duration: 0.0,
            speaker_person_mapping: HashMap::new(),
            confidence: 0.5,
            fallback_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualValidationResult {
    pub face_detected: bool,
    pub speaker_visible: bool,
    pub visual_score: f64, // 0.0 to 1.0
    pub is_acceptable: bool,
    pub issues: Vec<String>,
}

#[allow(async_fn_in_trait)]
pub trait ActiveSpeakerDetector: Send + Sync {
    fn name(&self) -> &str;
    async fn detect_active_speakers(
        &self,
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
        transcript: Option<&NormalizedTranscript>,
    ) -> Result<ActiveSpeakerTimeline>;
}

// =========================================================================
// 2. NVIDIA ACTIVE SPEAKER DETECTION (NIM / NVCF SPECIFICATION)
// =========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaDiarizationSegment {
    pub speaker_id: String,
    pub start: f64,
    pub end: f64,
}

/// Raw per-face/speaker detection emitted by NVIDIA Active Speaker Detection NIM
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvidiaRawDetection {
    #[serde(default, alias = "id", alias = "track_id")]
    pub face_id: Option<usize>,

    #[serde(
        default,
        alias = "bbox",
        alias = "bounding_box",
        alias = "face_bbox",
        alias = "box"
    )]
    pub speaker_bbox: Option<Vec<f64>>,

    #[serde(
        default,
        alias = "speaker_id",
        alias = "speaker",
        alias = "diarization_speaker_id"
    )]
    pub diarized_speaker_id: Option<String>,

    #[serde(
        default,
        alias = "speaking",
        alias = "speech_detected",
        alias = "active_speaker"
    )]
    pub is_speaking: Option<bool>,

    #[serde(
        default,
        alias = "score",
        alias = "speaking_confidence",
        alias = "speech_prob",
        alias = "prob"
    )]
    pub confidence: Option<f64>,

    #[serde(default, alias = "face_score", alias = "detection_confidence")]
    pub face_confidence: Option<f64>,
}

/// Raw per-frame record emitted by NVIDIA Active Speaker Detection NIM
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvidiaRawFrame {
    #[serde(
        default,
        alias = "time",
        alias = "timestamp_sec",
        alias = "time_offset",
        alias = "t"
    )]
    pub timestamp: f64,

    /// Multiple faces/speakers per frame
    #[serde(default, alias = "faces", alias = "detections", alias = "speakers")]
    pub faces: Vec<NvidiaRawDetection>,

    /// Direct flat detection fields if frame contains a single detection
    #[serde(flatten)]
    pub flat: NvidiaRawDetection,
}

/// Top-level response container from NVIDIA NIM
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvidiaAsdResponse {
    #[serde(default, alias = "frame_detections", alias = "predictions", alias = "data")]
    pub frames: Vec<NvidiaRawFrame>,

    #[serde(default)]
    pub status: Option<String>,
}

/// Simplified representation for backward compatibility and test ergonomics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvidiaPerFrameSpeaker {
    pub timestamp: f64,
    pub speaker_bbox: Option<[f64; 4]>,
    pub diarized_speaker_id: Option<String>,
    pub face_id: Option<usize>,
    pub is_speaking: bool,
    pub confidence: f64,
}

impl From<NvidiaPerFrameSpeaker> for NvidiaRawFrame {
    fn from(s: NvidiaPerFrameSpeaker) -> Self {
        Self {
            timestamp: s.timestamp,
            faces: vec![],
            flat: NvidiaRawDetection {
                face_id: s.face_id,
                speaker_bbox: s.speaker_bbox.map(|b| b.to_vec()),
                diarized_speaker_id: s.diarized_speaker_id,
                is_speaking: Some(s.is_speaking),
                confidence: Some(s.confidence),
                face_confidence: Some(s.confidence),
            },
        }
    }
}

// -------------------------------------------------------------------------
// CONVERSION LAYER STEP 1 & 2 DATA STRUCTURES
// -------------------------------------------------------------------------

/// Normalized per-frame speaker detection (Conversion Layer Step 1 output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedFrameDetection {
    pub timestamp_sec: f64,          // Relative to slice [0.0, duration_sec]
    pub absolute_timestamp_sec: f64, // Absolute video time (start_sec + timestamp_sec)
    pub speaker_bbox: Option<[f64; 4]>, // [x, y, w, h] normalized in [0.0, 1.0]
    pub bbox_center: Option<(f64, f64)>, // (cx, cy)
    pub raw_face_id: Option<usize>,
    pub diarized_speaker_id: Option<String>,
    pub is_speaking: bool,
    pub confidence: f64,
}

/// Associated detection with resolved visual Person identity (Conversion Layer Step 2 output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssociatedFrameDetection {
    pub timestamp_sec: f64,
    pub absolute_timestamp_sec: f64,
    pub person_id: usize, // 1-indexed (Person 1, Person 2, ...)
    pub speaker_bbox: Option<[f64; 4]>,
    pub bbox_center: Option<(f64, f64)>,
    pub diarized_speaker_id: Option<String>,
    pub is_speaking: bool,
    pub confidence: f64,
}

#[derive(Debug, Clone, Default)]
pub struct AssociatedSpeakerData {
    pub detections: Vec<AssociatedFrameDetection>,
    pub speaker_person_mapping: HashMap<String, usize>,
    pub confidence: f64,
}

#[derive(Debug, Deserialize)]
struct NvcfAssetResponse {
    #[serde(rename = "assetId")]
    asset_id: String,
    #[serde(rename = "uploadUrl")]
    upload_url: String,
}

#[derive(Debug, Clone)]
pub struct NvidiaAsdDetector {
    endpoint: String,
    api_key: String,
    function_id: Option<String>,
    grpc_target: String,
}

impl NvidiaAsdDetector {
    pub fn try_new() -> Result<Self> {
        let key = credentials::get(credentials::NVIDIA)?
            .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
            .ok_or_else(|| anyhow!("NVIDIA API key not configured"))?;

        let clean_key = key.trim().to_string();
        if clean_key.is_empty() {
            return Err(anyhow!("NVIDIA API key is empty"));
        }

        let function_id = credentials::get(credentials::NVIDIA_FUNCTION_ID)?
            .or_else(|| std::env::var("NVIDIA_ASD_FUNCTION_ID").ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let endpoint = std::env::var("NVIDIA_ASD_ENDPOINT")
            .unwrap_or_else(|_| "https://api.nvcf.nvidia.com/v2/nvcf".to_string());

        let grpc_target = std::env::var("NVIDIA_ASD_GRPC_TARGET")
            .unwrap_or_else(|_| "grpc.nvcf.nvidia.com:443".to_string());

        Ok(Self {
            endpoint,
            api_key: clean_key,
            function_id,
            grpc_target,
        })
    }

    pub fn with_config(
        endpoint: &str,
        api_key: &str,
        function_id: Option<String>,
        grpc_target: &str,
    ) -> Self {
        Self {
            endpoint: endpoint.trim().to_string(),
            api_key: api_key.trim().to_string(),
            function_id,
            grpc_target: grpc_target.trim().to_string(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn function_id(&self) -> Option<&str> {
        self.function_id.as_deref()
    }

    pub fn grpc_target(&self) -> &str {
        &self.grpc_target
    }
}

/// Extracts an optimized, lightweight H.264 MP4 video slice with AAC audio for NVIDIA ASD inference.
/// This prevents uploading heavy 4K/raw media while ensuring the model receives required visual/audio streams.
pub fn extract_asd_video_slice(
    source_path: &str,
    start_sec: f64,
    duration_sec: f64,
    output_path: &Path,
) -> Result<()> {
    let ffmpeg_bin = resolve_binary("ffmpeg");
    let mut cmd = Command::new(&ffmpeg_bin);
    cmd.arg("-nostdin");
    cmd.args(["-y", "-ss", &format!("{start_sec:.3}"), "-i", source_path]);
    cmd.args([
        "-t",
        &format!("{duration_sec:.3}"),
        "-vf",
        "scale='min(1280,iw)':-2",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "28",
        "-c:a",
        "aac",
        "-b:a",
        "64k",
        "-pix_fmt",
        "yuv420p",
    ]);
    cmd.arg(output_path);

    let output = cmd.output().context("Executing FFmpeg for ASD video slice")?;
    if !output.status.success() {
        return Err(anyhow!(
            "FFmpeg failed to extract ASD slice: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

// -------------------------------------------------------------------------
// CONVERSION LAYER IMPLEMENTATION (5 STAGES)
// -------------------------------------------------------------------------

/// Normalizes bounding box coordinates from varying representations:
/// - [x1, y1, x2, y2] corners vs [x, y, w, h] dimensions
/// - Raw pixel space (> 1.0) vs normalized [0.0, 1.0]
pub fn normalize_bounding_box(raw_box: &[f64]) -> (Option<[f64; 4]>, Option<(f64, f64)>) {
    if raw_box.len() < 4 {
        return (None, None);
    }

    let mut b0 = raw_box[0];
    let mut b1 = raw_box[1];
    let mut b2 = raw_box[2];
    let mut b3 = raw_box[3];

    // If raw coordinates appear in pixel units (e.g. 1920x1080 or 1280x720)
    if b0 > 1.5 || b1 > 1.5 || b2 > 1.5 || b3 > 1.5 {
        let norm_w = if b2 > 1280.0 { 1920.0 } else if b2 > 960.0 { 1280.0 } else { 960.0 };
        let norm_h = if b3 > 720.0 { 1080.0 } else if b3 > 540.0 { 720.0 } else { 540.0 };
        b0 = (b0 / norm_w).clamp(0.0, 1.0);
        b1 = (b1 / norm_h).clamp(0.0, 1.0);
        b2 = (b2 / norm_w).clamp(0.0, 1.0);
        b3 = (b3 / norm_h).clamp(0.0, 1.0);
    }

    // Detect if representation is [x1, y1, x2, y2]
    let (x, y, w, h) = if b2 > b0 && b3 > b1 && (b0 + b2 > 1.05 || b1 + b3 > 1.05) {
        (b0, b1, b2 - b0, b3 - b1)
    } else {
        (b0, b1, b2, b3)
    };

    let cx = (x + w * 0.5).clamp(0.0, 1.0);
    let cy = (y + h * 0.5).clamp(0.0, 1.0);
    let clamped_bbox = [
        x.clamp(0.0, 1.0),
        y.clamp(0.0, 1.0),
        w.clamp(0.0, 1.0),
        h.clamp(0.0, 1.0),
    ];

    (Some(clamped_bbox), Some((cx, cy)))
}

/// Conversion Layer Step 1: Normalizes raw NVIDIA frames into uniform timeline detections
pub fn normalize_nvidia_per_frame_detections(
    raw_frames: &[NvidiaRawFrame],
    start_sec: f64,
    duration_sec: f64,
) -> Vec<NormalizedFrameDetection> {
    let mut normalized = Vec::new();

    for frame in raw_frames {
        let t_sec = if frame.timestamp > 1000.0 && duration_sec < 120.0 {
            frame.timestamp / 1000.0
        } else {
            frame.timestamp
        };
        let rel_t = t_sec.clamp(0.0, duration_sec);
        let abs_t = start_sec + rel_t;

        let detections: Vec<&NvidiaRawDetection> = if !frame.faces.is_empty() {
            frame.faces.iter().collect()
        } else if frame.flat.speaker_bbox.is_some()
            || frame.flat.face_id.is_some()
            || frame.flat.is_speaking.is_some()
            || frame.flat.diarized_speaker_id.is_some()
        {
            vec![&frame.flat]
        } else {
            vec![]
        };

        for det in detections {
            let (bbox, center) = if let Some(ref raw_b) = det.speaker_bbox {
                normalize_bounding_box(raw_b)
            } else {
                (None, None)
            };

            let conf = det
                .confidence
                .or(det.face_confidence)
                .unwrap_or(0.85)
                .clamp(0.0, 1.0);

            let is_speaking = det.is_speaking.unwrap_or(conf >= 0.55);

            let diarized_id = det
                .diarized_speaker_id
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            normalized.push(NormalizedFrameDetection {
                timestamp_sec: rel_t,
                absolute_timestamp_sec: abs_t,
                speaker_bbox: bbox,
                bbox_center: center,
                raw_face_id: det.face_id,
                diarized_speaker_id: diarized_id,
                is_speaking,
                confidence: conf,
            });
        }
    }

    normalized.sort_by(|a, b| a.timestamp_sec.total_cmp(&b.timestamp_sec));
    normalized
}

/// Calculates 2D IoU (Intersection over Union) between two normalized [x, y, w, h] boxes
pub fn compute_bounding_box_iou(box1: &[f64; 4], box2: &[f64; 4]) -> f64 {
    let x1_min = box1[0];
    let y1_min = box1[1];
    let x1_max = box1[0] + box1[2];
    let y1_max = box1[1] + box1[3];

    let x2_min = box2[0];
    let y2_min = box2[1];
    let x2_max = box2[0] + box2[2];
    let y2_max = box2[1] + box2[3];

    let inter_xmin = x1_min.max(x2_min);
    let inter_ymin = y1_min.max(y2_min);
    let inter_xmax = x1_max.min(x2_max);
    let inter_ymax = y1_max.min(y2_max);

    let inter_w = (inter_xmax - inter_xmin).max(0.0);
    let inter_h = (inter_ymax - inter_ymin).max(0.0);
    let inter_area = inter_w * inter_h;

    let area1 = (box1[2] * box1[3]).max(0.0);
    let area2 = (box2[2] * box2[3]).max(0.0);
    let union_area = area1 + area2 - inter_area;

    if union_area <= 1e-6 {
        0.0
    } else {
        (inter_area / union_area).clamp(0.0, 1.0)
    }
}

/// Finds the nearest keyframe for a visual PersonTrack at timestamp t
pub fn get_person_keyframe_at<'a>(
    person: &'a crate::media::face_tracker::VisionPersonTrack,
    t: f64,
) -> Option<&'a crate::media::face_tracker::VisionKeyframe> {
    person
        .keyframes
        .iter()
        .min_by(|a, b| (a.t - t).abs().total_cmp(&(b.t - t).abs()))
}

/// Computes frame-level spatial affinity between a normalized detection and a ClipOn PersonTrack
pub fn compute_detection_person_affinity(
    det: &NormalizedFrameDetection,
    person: &crate::media::face_tracker::VisionPersonTrack,
) -> f64 {
    let kf = match get_person_keyframe_at(person, det.absolute_timestamp_sec) {
        Some(k) if k.visible && (k.t - det.absolute_timestamp_sec).abs() <= 1.2 => k,
        _ => return 0.0,
    };

    let w = if kf.width > 0.01 { kf.width } else { 0.18 };
    let h = if kf.height > 0.01 { kf.height } else { 0.24 };
    let kf_box = [
        (kf.x - w * 0.5).clamp(0.0, 1.0),
        (kf.y - h * 0.5).clamp(0.0, 1.0),
        w,
        h,
    ];

    let iou = if let Some(ref det_box) = det.speaker_bbox {
        compute_bounding_box_iou(det_box, &kf_box)
    } else {
        0.0
    };

    let center_dist = if let Some((cx, cy)) = det.bbox_center {
        ((cx - kf.x).powi(2) + (cy - kf.y).powi(2)).sqrt()
    } else if let Some(ref det_box) = det.speaker_bbox {
        let det_cx = det_box[0] + det_box[2] * 0.5;
        let det_cy = det_box[1] + det_box[3] * 0.5;
        ((det_cx - kf.x).powi(2) + (det_cy - kf.y).powi(2)).sqrt()
    } else {
        1.0
    };

    if center_dist > 0.40 && iou < 0.05 {
        0.0
    } else {
        let iou_weight = iou * 3.0;
        let dist_weight = (1.0 - center_dist * 2.5).max(0.0);
        iou_weight + dist_weight
    }
}

/// Builds explicit track-level association from NVIDIA face IDs to ClipOn PersonTrack IDs
/// Using bounding box IoU, temporal overlap aggregation, and optimal bipartite matching.
pub fn associate_nvidia_faces_with_clipon_tracks(
    normalized_detections: &[NormalizedFrameDetection],
    tracked_people: &[crate::media::face_tracker::VisionPersonTrack],
) -> HashMap<usize, usize> {
    let mut nvidia_to_clipon: HashMap<usize, usize> = HashMap::new();
    if normalized_detections.is_empty() {
        return nvidia_to_clipon;
    }

    // Collect all distinct NVIDIA face IDs
    let mut unique_nv_ids: Vec<usize> = Vec::new();
    for det in normalized_detections {
        if let Some(id) = det.raw_face_id {
            if !unique_nv_ids.contains(&id) {
                unique_nv_ids.push(id);
            }
        }
    }

    if unique_nv_ids.is_empty() {
        return nvidia_to_clipon;
    }

    // If ClipOn has no tracked people (fallback mode)
    if tracked_people.is_empty() {
        // Calculate average center X for each NVIDIA face ID
        let mut face_avg_x: Vec<(usize, f64)> = unique_nv_ids
            .iter()
            .map(|&nv_id| {
                let centers: Vec<f64> = normalized_detections
                    .iter()
                    .filter(|d| d.raw_face_id == Some(nv_id))
                    .filter_map(|d| d.bbox_center.map(|(cx, _)| cx))
                    .collect();
                let avg = if centers.is_empty() {
                    0.5
                } else {
                    centers.iter().sum::<f64>() / centers.len() as f64
                };
                (nv_id, avg)
            })
            .collect();
        // Sort left-to-right: leftmost becomes Person 1, next becomes Person 2, etc.
        face_avg_x.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (idx, (nv_id, _)) in face_avg_x.iter().enumerate() {
            nvidia_to_clipon.insert(*nv_id, idx + 1);
        }
        return nvidia_to_clipon;
    }

    // Compute cumulative temporal overlap & affinity matrix: A(nv_id, clipon_person_id)
    let mut affinity_matrix: HashMap<(usize, usize), f64> = HashMap::new();

    for &nv_id in &unique_nv_ids {
        for person in tracked_people {
            for det in normalized_detections {
                if det.raw_face_id == Some(nv_id) {
                    let aff = compute_detection_person_affinity(det, person);
                    if aff > 0.05 {
                        *affinity_matrix.entry((nv_id, person.id)).or_insert(0.0) += aff;
                    }
                }
            }
        }
    }

    // Solve optimal bipartite assignment
    if unique_nv_ids.len() == 2 && tracked_people.len() >= 2 {
        let f0 = unique_nv_ids[0];
        let f1 = unique_nv_ids[1];
        let p0 = tracked_people[0].id;
        let p1 = tracked_people[1].id;

        let score_a = affinity_matrix.get(&(f0, p0)).copied().unwrap_or(0.0)
            + affinity_matrix.get(&(f1, p1)).copied().unwrap_or(0.0);
        let score_b = affinity_matrix.get(&(f0, p1)).copied().unwrap_or(0.0)
            + affinity_matrix.get(&(f1, p0)).copied().unwrap_or(0.0);

        if score_a >= score_b {
            nvidia_to_clipon.insert(f0, p0);
            nvidia_to_clipon.insert(f1, p1);
        } else {
            nvidia_to_clipon.insert(f0, p1);
            nvidia_to_clipon.insert(f1, p0);
        }
    } else {
        // General N x M bipartite matching via greedy highest-affinity selection
        let mut candidates: Vec<(usize, usize, f64)> = Vec::new();
        for &nv_id in &unique_nv_ids {
            for person in tracked_people {
                let score = affinity_matrix.get(&(nv_id, person.id)).copied().unwrap_or(0.0);
                if score > 0.0 {
                    candidates.push((nv_id, person.id, score));
                }
            }
        }
        candidates.sort_by(|a, b| b.2.total_cmp(&a.2));

        let mut assigned_nv: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut assigned_clipon: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for (nv_id, person_id, _) in candidates {
            if !assigned_nv.contains(&nv_id) && !assigned_clipon.contains(&person_id) {
                nvidia_to_clipon.insert(nv_id, person_id);
                assigned_nv.insert(nv_id);
                assigned_clipon.insert(person_id);
            }
        }

        // For any remaining unassigned NVIDIA face IDs, fallback to spatial proximity
        for &nv_id in &unique_nv_ids {
            if !nvidia_to_clipon.contains_key(&nv_id) {
                let centers: Vec<f64> = normalized_detections
                    .iter()
                    .filter(|d| d.raw_face_id == Some(nv_id))
                    .filter_map(|d| d.bbox_center.map(|(cx, _)| cx))
                    .collect();
                let avg_cx = if centers.is_empty() {
                    0.5
                } else {
                    centers.iter().sum::<f64>() / centers.len() as f64
                };

                let best_person = tracked_people
                    .iter()
                    .min_by(|a, b| {
                        let avg_a = a
                            .keyframes
                            .iter()
                            .map(|k| k.x)
                            .sum::<f64>()
                            / a.keyframes.len().max(1) as f64;
                        let avg_b = b
                            .keyframes
                            .iter()
                            .map(|k| k.x)
                            .sum::<f64>()
                            / b.keyframes.len().max(1) as f64;
                        (avg_a - avg_cx).abs().total_cmp(&(avg_b - avg_cx).abs())
                    })
                    .map(|p| p.id)
                    .unwrap_or(tracked_people[0].id);

                nvidia_to_clipon.insert(nv_id, best_person);
            }
        }
    }

    nvidia_to_clipon
}

/// Fallback matching when detection lacks raw_face_id or has unmatched track
fn match_detection_to_clipon_person(
    det: &NormalizedFrameDetection,
    tracked_people: &[crate::media::face_tracker::VisionPersonTrack],
) -> usize {
    if !tracked_people.is_empty() {
        let mut best_affinity = 0.0;
        let mut best_id = None;

        for person in tracked_people {
            let aff = compute_detection_person_affinity(det, person);
            if aff > best_affinity {
                best_affinity = aff;
                best_id = Some(person.id);
            }
        }

        if let Some(id) = best_id {
            if best_affinity > 0.15 {
                return id;
            }
        }

        // Horizontal center fallback
        if let Some((cx, _)) = det.bbox_center {
            let closest = tracked_people.iter().min_by(|a, b| {
                let avg_a = a.keyframes.iter().map(|k| k.x).sum::<f64>()
                    / a.keyframes.len().max(1) as f64;
                let avg_b = b.keyframes.iter().map(|k| k.x).sum::<f64>()
                    / b.keyframes.len().max(1) as f64;
                (avg_a - cx).abs().total_cmp(&(avg_b - cx).abs())
            });
            if let Some(p) = closest {
                return p.id;
            }
        }

        tracked_people[0].id
    } else {
        // Fallback when no visual tracks exist
        if let Some((cx, _)) = det.bbox_center {
            if cx < 0.50 { 1 } else { 2 }
        } else {
            1
        }
    }
}

/// Conversion Layer Step 2: Associates NVIDIA detections with ClipOn visual PersonTrack identities
/// Uses explicit track-level bounding box IoU, temporal overlap aggregation, and optimal bipartite matching.
pub fn associate_faces_and_persons(
    normalized_detections: &[NormalizedFrameDetection],
    face_tracker_opt: Option<&FaceTrackerResult>,
) -> AssociatedSpeakerData {
    if normalized_detections.is_empty() {
        return AssociatedSpeakerData::default();
    }

    let tracked_people = face_tracker_opt.map(|f| f.people()).unwrap_or(&[]);

    // 1. Build explicit Track-Level Association from NVIDIA face IDs to ClipOn PersonTracks
    let nvidia_to_clipon = associate_nvidia_faces_with_clipon_tracks(
        normalized_detections,
        tracked_people,
    );

    // 2. Map detections to resolved ClipOn person IDs and collect diarization co-occurrence votes
    let mut initial_associated: Vec<(usize, &NormalizedFrameDetection)> = Vec::new();
    let mut co_occurrence_votes: HashMap<(String, usize), f64> = HashMap::new();

    for det in normalized_detections {
        let resolved_person_id = if let Some(rf_id) = det.raw_face_id {
            if let Some(&p_id) = nvidia_to_clipon.get(&rf_id) {
                p_id
            } else {
                match_detection_to_clipon_person(det, tracked_people)
            }
        } else {
            match_detection_to_clipon_person(det, tracked_people)
        };

        if det.is_speaking {
            if let Some(ref spk) = det.diarized_speaker_id {
                *co_occurrence_votes
                    .entry((spk.clone(), resolved_person_id))
                    .or_insert(0.0) += det.confidence;
            }
        }

        initial_associated.push((resolved_person_id, det));
    }

    // 3. Synthesize audio diarization -> person mapping from co-occurrence votes
    let mut speaker_person_mapping: HashMap<String, usize> = HashMap::new();
    let mut speaker_best_votes: HashMap<String, (usize, f64)> = HashMap::new();

    for ((spk, pid), vote_weight) in co_occurrence_votes {
        let entry = speaker_best_votes.entry(spk).or_insert((pid, vote_weight));
        if vote_weight > entry.1 {
            *entry = (pid, vote_weight);
        }
    }
    for (spk, (pid, _)) in speaker_best_votes {
        speaker_person_mapping.insert(spk, pid);
    }

    // 4. Align detections with resolved audio diarization where spatial cues were ambiguous
    let mut final_detections = Vec::new();
    for (init_pid, det) in initial_associated {
        let pid = if let Some(ref spk) = det.diarized_speaker_id {
            if let Some(&voted_pid) = speaker_person_mapping.get(spk) {
                if det.speaker_bbox.is_none() && det.bbox_center.is_none() {
                    voted_pid
                } else {
                    init_pid
                }
            } else {
                init_pid
            }
        } else {
            init_pid
        };

        final_detections.push(AssociatedFrameDetection {
            timestamp_sec: det.timestamp_sec,
            absolute_timestamp_sec: det.absolute_timestamp_sec,
            person_id: pid,
            speaker_bbox: det.speaker_bbox,
            bbox_center: det.bbox_center,
            diarized_speaker_id: det.diarized_speaker_id.clone(),
            is_speaking: det.is_speaking,
            confidence: det.confidence,
        });
    }

    let avg_confidence = if final_detections.is_empty() {
        0.80
    } else {
        final_detections.iter().map(|d| d.confidence).sum::<f64>()
            / final_detections.len() as f64
    };

    AssociatedSpeakerData {
        detections: final_detections,
        speaker_person_mapping,
        confidence: avg_confidence.clamp(0.0, 1.0),
    }
}

/// Conversion Layer Step 3 & 4:
/// Applies temporal morphological closing & opening smoothing to active speaker detections,
/// then constructs continuous ActiveSpeakerSegment timeline intervals.
pub fn smooth_and_build_active_speaker_segments(
    detections: &[AssociatedFrameDetection],
    start_sec: f64,
    _duration_sec: f64,
) -> Vec<ActiveSpeakerSegment> {
    if detections.is_empty() {
        return Vec::new();
    }

    let mut speaking_dets: Vec<&AssociatedFrameDetection> =
        detections.iter().filter(|d| d.is_speaking).collect();
    speaking_dets.sort_by(|a, b| a.timestamp_sec.total_cmp(&b.timestamp_sec));

    if speaking_dets.is_empty() {
        return Vec::new();
    }

    // Temporal clustering with Morphological Closing (Bridge speech pauses <= 0.40s)
    let max_inter_word_gap = 0.40f64;
    let mut raw_segments: Vec<ActiveSpeakerSegment> = Vec::new();

    for d in speaking_dets {
        let abs_t = start_sec + d.timestamp_sec;
        let conf = d.confidence.clamp(0.0, 1.0);

        if let Some(last) = raw_segments.last_mut() {
            if last.person_id == d.person_id && (abs_t - last.end) <= max_inter_word_gap {
                last.end = abs_t + 0.05;
                last.confidence = (last.confidence * 0.7) + (conf * 0.3);
                if last.speaker_label.is_none() && d.diarized_speaker_id.is_some() {
                    last.speaker_label = d.diarized_speaker_id.clone();
                }
                continue;
            }
        }

        raw_segments.push(ActiveSpeakerSegment {
            start: abs_t,
            end: abs_t + 0.05,
            person_id: d.person_id,
            confidence: conf,
            speaker_label: d.diarized_speaker_id.clone(),
        });
    }

    // Morphological Opening: Prune spurious noise spikes (< 0.15s unless high confidence >= 0.90)
    let min_valid_duration = 0.15f64;
    let mut smoothed_segments: Vec<ActiveSpeakerSegment> = Vec::new();

    for seg in raw_segments {
        let seg_dur = seg.end - seg.start;
        if seg_dur >= min_valid_duration || seg.confidence >= 0.90 || (seg_dur >= 0.07 && seg.confidence >= 0.85) {
            smoothed_segments.push(seg);
        }
    }

    // Overlap resolution for clean cut transitions
    if smoothed_segments.len() > 1 {
        for i in 0..(smoothed_segments.len() - 1) {
            let next_start = smoothed_segments[i + 1].start;
            if smoothed_segments[i].end > next_start {
                let mid = (smoothed_segments[i].end + next_start) * 0.5;
                smoothed_segments[i].end = mid;
                smoothed_segments[i + 1].start = mid;
            }
        }
    }

    smoothed_segments
}

/// The unified 5-stage conversion pipeline:
/// NVIDIA per-frame results -> normalize -> Face/Person association -> temporal smoothing -> ActiveSpeakerSegment -> ActiveSpeakerTimeline
pub fn convert_nvidia_frames_to_timeline(
    raw_frames: &[NvidiaRawFrame],
    start_sec: f64,
    duration_sec: f64,
    face_tracker_opt: Option<&FaceTrackerResult>,
) -> ActiveSpeakerTimeline {
    // Stage 1: Normalize
    let normalized = normalize_nvidia_per_frame_detections(raw_frames, start_sec, duration_sec);

    // Stage 2: Face/Person Association
    let associated = associate_faces_and_persons(&normalized, face_tracker_opt);

    // Stage 3 & 4: Temporal Smoothing & ActiveSpeakerSegment Generation
    let segments = smooth_and_build_active_speaker_segments(
        &associated.detections,
        start_sec,
        duration_sec,
    );

    let avg_confidence = if segments.is_empty() {
        associated.confidence
    } else {
        (segments.iter().map(|s| s.confidence).sum::<f64>() / segments.len() as f64)
            .clamp(0.1, 1.0)
    };

    ActiveSpeakerTimeline {
        segments,
        provider: "nvidia_api".to_string(),
        source_duration: duration_sec,
        speaker_person_mapping: associated.speaker_person_mapping,
        confidence: avg_confidence,
        fallback_reason: None,
    }
}

/// Robust parser for NVIDIA Active Speaker Detection NIM and NVCF responses.
/// Handles standard NIM envelopes, results wrappers, top-level arrays, and legacy previews.
pub fn parse_nvidia_asd_response(response_body: &str) -> Result<Vec<NvidiaRawFrame>> {
    // 1. Try standard { "frames": [...] }
    if let Ok(resp) = serde_json::from_str::<NvidiaAsdResponse>(response_body) {
        if !resp.frames.is_empty() {
            return Ok(resp.frames);
        }
    }

    // 2. Try nested { "results": { "frames": [...] } }
    #[derive(Deserialize)]
    struct NestedResults {
        results: NvidiaAsdResponse,
    }
    if let Ok(nested) = serde_json::from_str::<NestedResults>(response_body) {
        if !nested.results.frames.is_empty() {
            return Ok(nested.results.frames);
        }
    }

    // 3. Try top-level array of frames [ { ... } ]
    if let Ok(frames) = serde_json::from_str::<Vec<NvidiaRawFrame>>(response_body) {
        if !frames.is_empty() {
            return Ok(frames);
        }
    }

    // 4. Try legacy mock format: { "active_speakers": [ ... ] }
    #[derive(Deserialize)]
    struct LegacyItem {
        start: f64,
        end: f64,
        #[serde(default)]
        speaker_id: Option<String>,
        #[serde(default)]
        face_id: Option<usize>,
        #[serde(default)]
        confidence: Option<f64>,
    }
    #[derive(Deserialize)]
    struct LegacyEnvelope {
        active_speakers: Vec<LegacyItem>,
    }
    if let Ok(legacy) = serde_json::from_str::<LegacyEnvelope>(response_body) {
        let mut synth_frames = Vec::new();
        for item in legacy.active_speakers {
            let mut t = item.start;
            while t <= item.end {
                synth_frames.push(NvidiaRawFrame {
                    timestamp: t,
                    faces: vec![],
                    flat: NvidiaRawDetection {
                        face_id: item.face_id,
                        speaker_bbox: None,
                        diarized_speaker_id: item.speaker_id.clone(),
                        is_speaking: Some(true),
                        confidence: item.confidence,
                        face_confidence: item.confidence,
                    },
                });
                t += 0.04;
            }
        }
        if !synth_frames.is_empty() {
            return Ok(synth_frames);
        }
    }

    Err(anyhow!(
        "Failed to parse NVIDIA Active Speaker Detection JSON response. Payload: {}",
        if response_body.len() > 300 {
            format!("{}...", &response_body[..300])
        } else {
            response_body.to_string()
        }
    ))
}

/// Backward compatibility entry point
pub fn aggregate_nvidia_frames_to_timeline(
    frames: &[NvidiaPerFrameSpeaker],
    start_sec: f64,
    duration_sec: f64,
) -> ActiveSpeakerTimeline {
    let raw: Vec<NvidiaRawFrame> = frames.iter().cloned().map(Into::into).collect();
    convert_nvidia_frames_to_timeline(&raw, start_sec, duration_sec, None)
}

/// Backward compatibility entry point with FaceTracker fusion
pub fn aggregate_nvidia_frames_to_timeline_with_faces(
    frames: &[NvidiaPerFrameSpeaker],
    start_sec: f64,
    duration_sec: f64,
    face_tracker_opt: Option<&FaceTrackerResult>,
) -> ActiveSpeakerTimeline {
    let raw: Vec<NvidiaRawFrame> = frames.iter().cloned().map(Into::into).collect();
    convert_nvidia_frames_to_timeline(&raw, start_sec, duration_sec, face_tracker_opt)
}

impl NvidiaAsdDetector {
    /// Detects active speakers via NVIDIA ASD NIM / NVCF with full visual FaceTracker fusion
    pub async fn detect_active_speakers_with_faces(
        &self,
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
        transcript: Option<&NormalizedTranscript>,
        face_tracker_opt: Option<&FaceTrackerResult>,
    ) -> Result<ActiveSpeakerTimeline> {
        let function_id = self.function_id.as_deref().ok_or_else(|| {
            anyhow!(
                "NVIDIA Active Speaker Detection requires an NVCF Function ID. \
                 Configure 'nvidia_function_id' in Settings or set NVIDIA_ASD_FUNCTION_ID."
            )
        })?;

        // 1. Prepare diarization JSON payload
        let mut diarization: Vec<NvidiaDiarizationSegment> = Vec::new();
        if let Some(t) = transcript {
            for seg in &t.segments {
                if seg.end >= start_sec && seg.start <= (start_sec + duration_sec) {
                    if let Some(ref spk) = seg.speaker {
                        diarization.push(NvidiaDiarizationSegment {
                            speaker_id: spk.clone(),
                            start: (seg.start - start_sec).max(0.0),
                            end: (seg.end - start_sec).min(duration_sec),
                        });
                    }
                }
            }
        }

        // 2. Extract lightweight H.264 MP4 video slice
        let temp_dir = std::env::temp_dir();
        let slice_filename = format!("clipon_asd_slice_{}.mp4", uuid::Uuid::new_v4());
        let slice_path = temp_dir.join(&slice_filename);

        extract_asd_video_slice(source_path, start_sec, duration_sec, &slice_path)?;

        // Ensure temp slice is deleted when function exits
        struct SliceCleaner(PathBuf);
        impl Drop for SliceCleaner {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleaner = SliceCleaner(slice_path.clone());

        let video_bytes = tokio::fs::read(&slice_path)
            .await
            .context("Reading extracted ASD video slice")?;

        let client = http_client::build_api_client(30);

        // 3. Upload video asset to NVIDIA Cloud Functions Asset API
        let asset_create_url = format!("{}/assets", self.endpoint);
        let asset_init_resp = client
            .post(&asset_create_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "contentType": "video/mp4",
                "description": "ClipOn ASD video slice"
            }))
            .send()
            .await
            .context("Initiating NVCF asset upload")?;

        if !asset_init_resp.status().is_success() {
            let status = asset_init_resp.status();
            let body = asset_init_resp.text().await.unwrap_or_default();
            return Err(anyhow!("NVCF Asset initialization failed ({status}): {body}"));
        }

        let asset_meta: NvcfAssetResponse = asset_init_resp
            .json()
            .await
            .context("Parsing NVCF asset metadata")?;

        // Upload video binary to presigned storage URL
        let upload_resp = client
            .put(&asset_meta.upload_url)
            .header("Content-Type", "video/mp4")
            .header("x-amz-meta-nvcf-asset-id", &asset_meta.asset_id)
            .body(video_bytes)
            .send()
            .await
            .context("Uploading video slice binary to NVCF asset store")?;

        if !upload_resp.status().is_success() {
            return Err(anyhow!(
                "Failed uploading video slice to NVCF storage: {}",
                upload_resp.status()
            ));
        }

        // 4. Invoke NVCF Active Speaker Detection Function
        let pexec_url = format!("{}/pexec/functions/{}", self.endpoint, function_id);
        let invoke_resp = client
            .post(&pexec_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("NVCF-INPUT-ASSET-REFERENCES", &asset_meta.asset_id)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "start_sec": 0.0,
                "duration_sec": duration_sec,
                "diarization": diarization
            }))
            .send()
            .await
            .context("Invoking NVCF Active Speaker Detection function")?;

        let mut final_response = invoke_resp;

        // If status 202 (Accepted / Pending), poll status endpoint
        if final_response.status().as_u16() == 202 {
            let status_url = final_response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            if let Some(poll_url) = status_url {
                let mut attempts = 0;
                while attempts < 30 {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    let poll_resp = client
                        .get(&poll_url)
                        .header("Authorization", format!("Bearer {}", self.api_key))
                        .send()
                        .await?;

                    if poll_resp.status().as_u16() != 202 {
                        final_response = poll_resp;
                        break;
                    }
                    attempts += 1;
                }
            }
        }

        if !final_response.status().is_success() {
            let status = final_response.status();
            let err_body = final_response.text().await.unwrap_or_default();
            return Err(anyhow!("NVCF execution failed ({status}): {err_body}"));
        }

        let response_text = final_response
            .text()
            .await
            .context("Reading NVIDIA ASD response body")?;

        let raw_frames = parse_nvidia_asd_response(&response_text)?;
        let timeline = convert_nvidia_frames_to_timeline(
            &raw_frames,
            start_sec,
            duration_sec,
            face_tracker_opt,
        );
        Ok(timeline)
    }
}

impl ActiveSpeakerDetector for NvidiaAsdDetector {
    fn name(&self) -> &str {
        "nvidia_api"
    }

    async fn detect_active_speakers(
        &self,
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
        transcript: Option<&NormalizedTranscript>,
    ) -> Result<ActiveSpeakerTimeline> {
        let face_result = FaceTracker::analyze(source_path, start_sec, duration_sec);
        self.detect_active_speakers_with_faces(
            source_path,
            start_sec,
            duration_sec,
            transcript,
            Some(&face_result),
        )
        .await
    }
}

// =========================================================================
// 3. LOCAL MULTIMODAL VISION-AUDIO FUSION DETECTOR (ROBUST FALLBACK)
// =========================================================================

#[derive(Debug, Default, Clone)]
pub struct LocalFusionDetector;

impl LocalFusionDetector {
    pub fn new() -> Self {
        Self
    }
}

/// Measures visual mouth/facial motion energy for a person during a speech interval using frame differences.
/// Samples a short window of low-res grayscale frames to compute inter-frame variance in the mouth bounding box.
pub fn measure_mouth_activity_motion(
    source_path: &str,
    sample_start: f64,
    sample_duration: f64,
    person_keyframe: Option<&crate::media::face_tracker::VisionKeyframe>,
) -> f64 {
    let kf = match person_keyframe {
        Some(k) if k.visible && k.width > 0.02 && k.height > 0.02 => k,
        _ => return 0.0,
    };

    let ffmpeg_bin = resolve_binary("ffmpeg");
    let mut cmd = Command::new(&ffmpeg_bin);
    cmd.arg("-nostdin");
    cmd.args(["-y", "-ss", &format!("{sample_start:.3}"), "-i", source_path]);
    cmd.args([
        "-t",
        &format!("{sample_duration:.3}"),
        "-vf",
        "fps=10,scale=320:180",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "gray",
        "pipe:1",
    ]);

    let output = match cmd.output() {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => o.stdout,
        _ => return 0.0,
    };

    let frame_size = 320 * 180;
    let num_frames = output.len() / frame_size;
    if num_frames < 2 {
        return 0.0;
    }

    // In Apple Vision face tracker: kf.x and kf.y are center coordinates normalized in [0.0, 1.0]
    let face_w = kf.width.clamp(0.04, 0.70);
    let face_h = kf.height.clamp(0.04, 0.70);
    let face_left = (kf.x - face_w * 0.5).clamp(0.0, 1.0);
    let face_top = (kf.y - face_h * 0.5).clamp(0.0, 1.0);

    // Mouth region is in the lower 40% of the face, horizontally centered within the face
    let mouth_x1 = ((face_left + face_w * 0.20) * 320.0).clamp(0.0, 319.0) as usize;
    let mouth_x2 = ((face_left + face_w * 0.80) * 320.0).clamp(0.0, 319.0) as usize;
    let mouth_y1 = ((face_top + face_h * 0.55) * 180.0).clamp(0.0, 179.0) as usize;
    let mouth_y2 = ((face_top + face_h * 0.95) * 180.0).clamp(0.0, 179.0) as usize;

    if mouth_x2 <= mouth_x1 || mouth_y2 <= mouth_y1 {
        return 0.0;
    }

    let mut total_diff = 0.0f64;
    let mut pixel_count = 0usize;

    for f in 1..num_frames {
        let prev_frame = &output[(f - 1) * frame_size..f * frame_size];
        let curr_frame = &output[f * frame_size..(f + 1) * frame_size];

        for y in mouth_y1..mouth_y2 {
            let row_offset = y * 320;
            for x in mouth_x1..mouth_x2 {
                let idx = row_offset + x;
                let diff = (curr_frame[idx] as f64 - prev_frame[idx] as f64).abs();
                total_diff += diff;
                pixel_count += 1;
            }
        }
    }

    if pixel_count == 0 {
        0.0
    } else {
        total_diff / pixel_count as f64
    }
}

/// Computes speaker ↔ person association using true temporal evidence:
/// 1. Audio speaker timeline (exact speech intervals for each speaker)
/// 2. Visible face tracks (who is on screen during each speech interval)
/// 3. Mouth/lip activity motion (who is moving their mouth when the audio is playing)
/// 4. Temporal consistency (bipartite optimal matching)
pub fn compute_temporal_speaker_person_mapping(
    source_path: &str,
    start_sec: f64,
    duration_sec: f64,
    transcript: &NormalizedTranscript,
    people: &[crate::media::face_tracker::VisionPersonTrack],
) -> (HashMap<String, usize>, f64) {
    if people.is_empty() {
        return (HashMap::new(), 0.50);
    }

    if people.len() == 1 {
        let mut mapping = HashMap::new();
        for seg in &transcript.segments {
            if let Some(ref spk) = seg.speaker {
                mapping.insert(spk.clone(), people[0].id);
            }
        }
        return (mapping, 0.92);
    }

    // Collect all relevant speech intervals per speaker
    let mut speaker_intervals: HashMap<String, Vec<(f64, f64)>> = HashMap::new();
    for seg in &transcript.segments {
        if seg.end >= start_sec && seg.start <= (start_sec + duration_sec) {
            if let Some(ref spk) = seg.speaker {
                let s = seg.start.max(start_sec);
                let e = seg.end.min(start_sec + duration_sec);
                if e > s {
                    speaker_intervals.entry(spk.clone()).or_default().push((s, e));
                }
            }
        }
    }

    if speaker_intervals.is_empty() {
        let mut mapping = HashMap::new();
        for (i, p) in people.iter().enumerate() {
            mapping.insert(format!("S{}", i + 1), p.id);
        }
        return (mapping, 0.60);
    }

    fn get_person_kf_at<'a>(
        p: &'a crate::media::face_tracker::VisionPersonTrack,
        t: f64,
    ) -> Option<&'a crate::media::face_tracker::VisionKeyframe> {
        p.keyframes.iter().min_by(|a, b| (a.t - t).abs().total_cmp(&(b.t - t).abs()))
    }

    // Accumulate temporal evidence matrix: E[speaker, person_id]
    let mut evidence_matrix: HashMap<(String, usize), f64> = HashMap::new();

    for (spk, intervals) in &speaker_intervals {
        for &(i_start, i_end) in intervals {
            let interval_dur = i_end - i_start;
            let sample_step = 0.30f64;
            let mut sample_t = i_start + 0.10;

            // 1. Temporal Visibility Evidence
            while sample_t <= i_end {
                let mut visible_persons: Vec<usize> = Vec::new();
                for p in people {
                    if let Some(kf) = get_person_kf_at(p, sample_t) {
                        if kf.visible && (kf.t - sample_t).abs() < 0.60 {
                            visible_persons.push(p.id);
                        }
                    }
                }

                if visible_persons.len() == 1 {
                    // Exclusive visibility: Person visible is very likely the active speaker
                    let sole_id = visible_persons[0];
                    *evidence_matrix.entry((spk.clone(), sole_id)).or_insert(0.0) += 2.5;

                    for p in people {
                        if p.id != sole_id {
                            *evidence_matrix.entry((spk.clone(), p.id)).or_insert(0.0) -= 1.5;
                        }
                    }
                } else if visible_persons.len() > 1 {
                    for &vid in &visible_persons {
                        *evidence_matrix.entry((spk.clone(), vid)).or_insert(0.0) += 0.4;
                    }
                }

                sample_t += sample_step;
            }

            // 2. Mouth / Lip Motion Activity Evidence
            // When multiple people are visible during sustained speech (>= 0.6s)
            if interval_dur >= 0.60
                && (source_path.ends_with(".mp4")
                    || source_path.ends_with(".mov")
                    || source_path.ends_with(".mkv"))
            {
                let sample_window_start = i_start + (interval_dur * 0.2);
                let sample_window_dur = (interval_dur * 0.6).clamp(0.4, 0.8);

                let mut person_motions: Vec<(usize, f64)> = Vec::new();
                for p in people {
                    if let Some(kf) = get_person_kf_at(p, sample_window_start) {
                        let motion = measure_mouth_activity_motion(
                            source_path,
                            sample_window_start,
                            sample_window_dur,
                            Some(kf),
                        );
                        person_motions.push((p.id, motion));
                    }
                }

                if person_motions.len() >= 2 {
                    person_motions.sort_by(|a, b| b.1.total_cmp(&a.1));
                    let (best_id, best_motion) = person_motions[0];
                    let (_second_id, second_motion) = person_motions[1];

                    if best_motion > 4.0 && (best_motion - second_motion) > 2.0 {
                        *evidence_matrix.entry((spk.clone(), best_id)).or_insert(0.0) +=
                            (best_motion - second_motion).min(10.0) * 1.5;
                    }
                }
            }
        }
    }

    // 3. Optimal Global Bipartite Matching
    let speaker_list: Vec<String> = speaker_intervals.keys().cloned().collect();
    let person_list: Vec<usize> = people.iter().map(|p| p.id).collect();

    let (best_mapping, confidence) = if speaker_list.len() == 2 && person_list.len() >= 2 {
        let s0 = &speaker_list[0];
        let s1 = &speaker_list[1];
        let p0 = person_list[0];
        let p1 = person_list[1];

        let score_perm_a = evidence_matrix.get(&(s0.clone(), p0)).copied().unwrap_or(0.0)
            + evidence_matrix.get(&(s1.clone(), p1)).copied().unwrap_or(0.0);
        let score_perm_b = evidence_matrix.get(&(s0.clone(), p1)).copied().unwrap_or(0.0)
            + evidence_matrix.get(&(s1.clone(), p0)).copied().unwrap_or(0.0);

        let mut mapping = HashMap::new();
        let (_margin, conf) = if score_perm_a >= score_perm_b {
            mapping.insert(s0.clone(), p0);
            mapping.insert(s1.clone(), p1);
            (score_perm_a - score_perm_b, 0.85 + (score_perm_a - score_perm_b).clamp(0.0, 10.0) * 0.012)
        } else {
            mapping.insert(s0.clone(), p1);
            mapping.insert(s1.clone(), p0);
            (score_perm_b - score_perm_a, 0.85 + (score_perm_b - score_perm_a).clamp(0.0, 10.0) * 0.012)
        };

        (mapping, conf.clamp(0.70, 0.98))
    } else {
        let mut mapping = HashMap::new();
        let mut available_people = person_list.clone();

        for spk in &speaker_list {
            if available_people.is_empty() {
                mapping.insert(spk.clone(), person_list[0]);
                continue;
            }

            let best_person = available_people
                .iter()
                .cloned()
                .max_by(|&a, &b| {
                    let score_a = evidence_matrix.get(&(spk.clone(), a)).copied().unwrap_or(0.0);
                    let score_b = evidence_matrix.get(&(spk.clone(), b)).copied().unwrap_or(0.0);
                    score_a.total_cmp(&score_b)
                })
                .unwrap_or(available_people[0]);

            mapping.insert(spk.clone(), best_person);
            if available_people.len() > 1 {
                available_people.retain(|&pid| pid != best_person);
            }
        }

        (mapping, 0.85)
    };

    (best_mapping, confidence)
}

impl LocalFusionDetector {
    pub async fn detect_active_speakers_with_faces(
        &self,
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
        transcript: Option<&NormalizedTranscript>,
        face_tracker_opt: Option<&FaceTrackerResult>,
    ) -> Result<ActiveSpeakerTimeline> {
        let local_vision;
        let vision_result = match face_tracker_opt {
            Some(v) => v,
            None => {
                local_vision = FaceTracker::analyze(source_path, start_sec, duration_sec);
                &local_vision
            }
        };
        let people = vision_result.people();

        let (speaker_mapping, confidence) = if let Some(t) = transcript {
            compute_temporal_speaker_person_mapping(
                source_path,
                start_sec,
                duration_sec,
                t,
                people,
            )
        } else {
            (HashMap::new(), 0.60)
        };

        let mut segments: Vec<ActiveSpeakerSegment> = Vec::new();

        if let Some(t) = transcript {
            for seg in &t.segments {
                if seg.end < start_sec || seg.start > (start_sec + duration_sec) {
                    continue;
                }
                let s_start = seg.start.max(start_sec);
                let s_end = seg.end.min(start_sec + duration_sec);
                if s_end <= s_start {
                    continue;
                }

                let p_id = if let Some(ref speaker) = seg.speaker {
                    speaker_mapping.get(speaker).copied().unwrap_or_else(|| {
                        people.first().map(|p| p.id).unwrap_or(1)
                    })
                } else {
                    people.first().map(|p| p.id).unwrap_or(1)
                };

                segments.push(ActiveSpeakerSegment {
                    start: s_start,
                    end: s_end,
                    person_id: p_id,
                    confidence,
                    speaker_label: seg.speaker.clone(),
                });
            }
        } else {
            segments.push(ActiveSpeakerSegment {
                start: start_sec,
                end: start_sec + duration_sec,
                person_id: 1,
                confidence: if vision_result.face_detected { 0.80 } else { 0.50 },
                speaker_label: None,
            });
        }

        // Apply temporal smoothing & gap bridging
        segments.sort_by(|a, b| a.start.total_cmp(&b.start));

        let mut smoothed_segments: Vec<ActiveSpeakerSegment> = Vec::new();
        for seg in segments {
            if let Some(last) = smoothed_segments.last_mut() {
                if last.person_id == seg.person_id && (seg.start - last.end) <= 0.40 {
                    last.end = seg.end;
                    if last.speaker_label.is_none() && seg.speaker_label.is_some() {
                        last.speaker_label = seg.speaker_label.clone();
                    }
                    continue;
                }
            }
            smoothed_segments.push(seg);
        }

        Ok(ActiveSpeakerTimeline {
            segments: smoothed_segments,
            provider: "local_vision_fusion".to_string(),
            source_duration: duration_sec,
            speaker_person_mapping: speaker_mapping,
            confidence,
            fallback_reason: None,
        })
    }
}

impl ActiveSpeakerDetector for LocalFusionDetector {
    fn name(&self) -> &str {
        "local_vision_fusion"
    }

    async fn detect_active_speakers(
        &self,
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
        transcript: Option<&NormalizedTranscript>,
    ) -> Result<ActiveSpeakerTimeline> {
        self.detect_active_speakers_with_faces(
            source_path,
            start_sec,
            duration_sec,
            transcript,
            None,
        )
        .await
    }
}

// =========================================================================
// 4. PERSISTENT CACHE & UNIFIED PIPELINE DISPATCHER
// =========================================================================

pub const ASD_ANALYSIS_VERSION: &str = "asd_fusion_v4";
pub const ASD_FACE_TRACKER_VERSION: &str = "apple_vision_v2";
pub const ASD_DIARIZATION_VERSION: &str = "diarize_v1";
pub const ASD_SPEAKING_THRESHOLD: f64 = 0.55;
pub const ASD_MIN_HOLD_SEC: f64 = 2.0;

/// Resolves the active NVIDIA ASD NIM model / function ID identifier
pub fn get_nvidia_asd_model_identifier() -> String {
    credentials::get(credentials::NVIDIA_FUNCTION_ID)
        .ok()
        .flatten()
        .or_else(|| std::env::var("NVIDIA_ASD_FUNCTION_ID").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default_nim_asd".to_string())
}

/// Computes the comprehensive, deterministic cache key for Active Speaker Detection.
/// Incorporates all 7 critical configuration parameters to guarantee cache freshness:
/// 1. Source media path & content fingerprint (size, mtime, byte digests)
/// 2. Video duration bounds (start_sec, duration_sec)
/// 3. Analysis algorithm version (asd_fusion_v4)
/// 4. NVIDIA ASD NIM model / function ID
/// 5. ASD speaking threshold (0.55) & hysteresis hold (2.0s)
/// 6. Diarization engine version (diarize_v1)
/// 7. Apple Vision face tracker version (apple_vision_v2)
pub fn compute_active_speaker_cache_key(
    source_path: &str,
    start_sec: f64,
    duration_sec: f64,
) -> String {
    let analyzer_str = format!("{}_{}", ASD_ANALYSIS_VERSION, ASD_FACE_TRACKER_VERSION);
    let model_str = format!("nim_{}", get_nvidia_asd_model_identifier());
    let params_str = format!(
        "thresh_{:.2}_hold_{:.1}_diar_{}",
        ASD_SPEAKING_THRESHOLD, ASD_MIN_HOLD_SEC, ASD_DIARIZATION_VERSION
    );

    AnalysisCache::compute_source_key_with_params(
        source_path,
        start_sec,
        duration_sec,
        &analyzer_str,
        &model_str,
        &params_str,
    )
}

pub async fn get_or_compute_active_speaker_timeline(
    source_path: &str,
    start_sec: f64,
    duration_sec: f64,
    transcript: Option<&NormalizedTranscript>,
) -> ActiveSpeakerTimeline {
    let cache = AnalysisCache::global();
    let cache_key = compute_active_speaker_cache_key(source_path, start_sec, duration_sec);

    // 1. Check persistent AnalysisCache
    if let Some(cached) = cache.get::<ActiveSpeakerTimeline>(&cache_key, "active_speaker") {
        if cached.provider == "nvidia_api" {
            println!(
                "\n======================================================================\n[ClipOn ASD] ACTIVE SPEAKER PROVIDER: NVIDIA ASD (cached)\nStatus: High-fidelity per-frame neural inference with Apple Vision face fusion.\nSegments: {}\n======================================================================\n",
                cached.segments.len()
            );
        } else {
            let reason = cached
                .fallback_reason
                .as_deref()
                .unwrap_or("No NVIDIA credentials configured");
            eprintln!(
                "\n======================================================================\n[ClipOn ASD] ACTIVE SPEAKER PROVIDER: Local fallback (cached)\nReason: {}\nQuality: Apple Vision face tracking + diarization temporal fusion.\n======================================================================\n",
                reason
            );
        }
        return cached;
    }

    // 2. Obtain FaceTracker visual tracking for ClipOn PersonTrack association
    let face_result = FaceTracker::analyze(source_path, start_sec, duration_sec);

    // 3. Attempt primary NVIDIA API Active Speaker Detection with FaceTracker fusion
    let (timeline_res, fallback_reason) = match NvidiaAsdDetector::try_new() {
        Ok(nvidia_detector) => {
            match nvidia_detector
                .detect_active_speakers_with_faces(
                    source_path,
                    start_sec,
                    duration_sec,
                    transcript,
                    Some(&face_result),
                )
                .await
            {
                Ok(timeline) => {
                    println!(
                        "\n======================================================================\n[ClipOn ASD] ACTIVE SPEAKER PROVIDER: NVIDIA ASD\nStatus: SUCCESS (per-frame neural ASD with Apple Vision face fusion)\nSegments: {}\nConfidence: {:.2}\n======================================================================\n",
                        timeline.segments.len(),
                        timeline.confidence
                    );
                    (Some(timeline), None)
                }
                Err(e) => {
                    let err_str = e.to_string();
                    eprintln!(
                        "\n======================================================================\n[ClipOn ASD] WARNING: NVIDIA ASD FAILED -> FALLING BACK TO LOCAL FUSION\nError: {}\nActive speaker provider is now: Local fallback\n======================================================================\n",
                        err_str
                    );
                    (None, Some(format!("NVIDIA inference failed: {}", err_str)))
                }
            }
        }
        Err(e) => {
            let reason = format!("NVIDIA credentials not configured or invalid: {}", e);
            eprintln!(
                "\n======================================================================\n[ClipOn ASD] NOTICE: ACTIVE SPEAKER PROVIDER: Local fallback\nReason: {}\nActive speaker provider is now: Local fallback (Apple Vision + Diarization)\n======================================================================\n",
                reason
            );
            (None, Some(reason))
        }
    };

    // 4. Fallback to Local Multimodal Vision-Audio Fusion
    let final_timeline = match timeline_res {
        Some(t) => t,
        None => {
            let local_detector = LocalFusionDetector::new();
            let mut local_tl = local_detector
                .detect_active_speakers_with_faces(
                    source_path,
                    start_sec,
                    duration_sec,
                    transcript,
                    Some(&face_result),
                )
                .await
                .unwrap_or_default();
            local_tl.fallback_reason = fallback_reason;
            local_tl
        }
    };

    // 5. Cache validated active-speaker timeline
    let _ = cache.put(&cache_key, "active_speaker", &final_timeline);

    final_timeline
}

// =========================================================================
// 5. HYSTERESIS, SMOOTH EASING & SMART SHOT SELECTION REFRAMING
// =========================================================================

/// Generates speaker-aware 9:16 crop keyframes with:
/// 1. Hysteresis: minimum hold duration (2.0s) to prevent erratic flipping.
/// 2. Smooth ease-in-out interpolation (0.5s) between speakers.
/// 3. Smart shot selection: wider two-person framing if speakers alternate rapidly (< 2.0s).
pub fn generate_speaker_aware_keyframes(
    timeline: &ActiveSpeakerTimeline,
    face_result: &FaceTrackerResult,
    clip_start: f64,
    clip_end: f64,
) -> Vec<TrackingKeyframe> {
    let duration = (clip_end - clip_start).max(0.1);
    let default_cx = face_result.avg_center_x.clamp(0.20, 0.80);
    let default_cy = 0.38f64;

    let people = face_result.people();

    let get_person_center_at = |pid: usize, t_abs: f64| -> f64 {
        if let Some(person) = people.iter().find(|p| p.id == pid) {
            if let Some(kf) = person
                .keyframes
                .iter()
                .min_by(|a, b| (a.t - t_abs).abs().total_cmp(&(b.t - t_abs).abs()))
            {
                return kf.x.clamp(0.20, 0.80);
            }
            if let Some(last_kf) = person.keyframes.last() {
                return last_kf.x.clamp(0.20, 0.80);
            }
        }
        if people.len() >= 2 {
            if pid == 1 {
                return 0.28;
            } else if pid == 2 {
                return 0.72;
            }
        }
        default_cx
    };

    let relevant_segments: Vec<&ActiveSpeakerSegment> = timeline
        .segments
        .iter()
        .filter(|s| s.end > clip_start && s.start < clip_end)
        .collect();

    if relevant_segments.is_empty() {
        return vec![TrackingKeyframe {
            t: 0.0,
            x: default_cx,
            y: default_cy,
        }];
    }

    struct HeldSegment {
        start: f64,
        end: f64,
        person_id: usize,
    }

    let mut held_segments: Vec<HeldSegment> = Vec::new();
    for seg in relevant_segments {
        let seg_start = (seg.start - clip_start).max(0.0);
        let seg_end = (seg.end - clip_start).min(duration);
        if seg_end <= seg_start {
            continue;
        }

        if let Some(last) = held_segments.last_mut() {
            if last.person_id == seg.person_id {
                last.end = seg_end;
            } else {
                let current_duration = last.end - last.start;
                let new_duration = seg_end - seg_start;

                if current_duration < 2.0 || new_duration < 1.5 {
                    if people.len() >= 2 {
                        last.person_id = 0; // Wide shot designation
                    }
                    last.end = seg_end;
                } else {
                    held_segments.push(HeldSegment {
                        start: seg_start,
                        end: seg_end,
                        person_id: seg.person_id,
                    });
                }
            }
        } else {
            held_segments.push(HeldSegment {
                start: seg_start,
                end: seg_end,
                person_id: seg.person_id,
            });
        }
    }

    if held_segments.is_empty() {
        return vec![TrackingKeyframe {
            t: 0.0,
            x: default_cx,
            y: default_cy,
        }];
    }

    let mut keyframes: Vec<TrackingKeyframe> = Vec::new();
    let transition_duration = 0.50f64;

    for (i, seg) in held_segments.iter().enumerate() {
        let seg_mid_t = clip_start + (seg.start + seg.end) * 0.5;
        let target_x = if seg.person_id == 0 {
            let x1 = get_person_center_at(1, seg_mid_t);
            let x2 = get_person_center_at(2, seg_mid_t);
            ((x1 + x2) / 2.0).clamp(0.25, 0.75)
        } else {
            get_person_center_at(seg.person_id, seg_mid_t)
        };

        if i == 0 {
            keyframes.push(TrackingKeyframe {
                t: 0.0,
                x: target_x,
                y: default_cy,
            });
        } else {
            let prev_x = keyframes.last().map(|k| k.x).unwrap_or(default_cx);
            if (prev_x - target_x).abs() > 0.04 {
                let trans_start = (seg.start - transition_duration * 0.5).max(0.0);
                let trans_end = (seg.start + transition_duration * 0.5).min(duration);

                keyframes.push(TrackingKeyframe {
                    t: trans_start,
                    x: prev_x,
                    y: default_cy,
                });
                keyframes.push(TrackingKeyframe {
                    t: trans_end,
                    x: target_x,
                    y: default_cy,
                });
            } else {
                keyframes.push(TrackingKeyframe {
                    t: seg.start,
                    x: target_x,
                    y: default_cy,
                });
            }
        }

        keyframes.push(TrackingKeyframe {
            t: seg.end,
            x: target_x,
            y: default_cy,
        });
    }

    if let Some(last) = keyframes.last().cloned() {
        if last.t < duration {
            keyframes.push(TrackingKeyframe {
                t: duration,
                x: last.x,
                y: last.y,
            });
        }
    }

    keyframes
}

// =========================================================================
// 6. VISUAL QUALITY VALIDATION
// =========================================================================

pub fn validate_clip_visuals(
    timeline: &ActiveSpeakerTimeline,
    face_result: &FaceTrackerResult,
    clip_start: f64,
    clip_end: f64,
) -> VisualValidationResult {
    let mut issues = Vec::new();
    let mut score = 0.90f64;

    if !face_result.face_detected {
        issues.push("No visible face detected in scene".to_string());
        score -= 0.35;
    }

    let relevant_segments: Vec<_> = timeline
        .segments
        .iter()
        .filter(|s| s.end > clip_start && s.start < clip_end)
        .collect();

    let speaker_visible = !relevant_segments.is_empty();
    if !speaker_visible {
        issues.push("Active speaker not visible during moment".to_string());
        score -= 0.25;
    }

    if let Some(height) = face_result.height {
        if height < 0.08 {
            issues.push("Face subject is too small/distant for vertical crop".to_string());
            score -= 0.15;
        }
    }

    let is_acceptable = score >= 0.50;

    VisualValidationResult {
        face_detected: face_result.face_detected,
        speaker_visible,
        visual_score: score.clamp(0.0, 1.0),
        is_acceptable,
        issues,
    }
}

// =========================================================================
// 7. TESTS
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nvidia_diarization_serialization() {
        let segs = vec![NvidiaDiarizationSegment {
            speaker_id: "S1".to_string(),
            start: 0.0,
            end: 5.0,
        }];
        let json = serde_json::to_string(&segs).expect("must serialize");
        assert!(json.contains("speaker_id"));
        assert!(json.contains("\"start\":0.0"));
    }

    #[test]
    fn test_nvidia_per_frame_parsing_and_aggregation() {
        let raw_frames = vec![
            NvidiaPerFrameSpeaker {
                timestamp: 0.04,
                speaker_bbox: Some([0.25, 0.20, 0.18, 0.24]),
                diarized_speaker_id: Some("S1".to_string()),
                face_id: Some(1),
                is_speaking: true,
                confidence: 0.94,
            },
            NvidiaPerFrameSpeaker {
                timestamp: 0.08,
                speaker_bbox: Some([0.25, 0.20, 0.18, 0.24]),
                diarized_speaker_id: Some("S1".to_string()),
                face_id: Some(1),
                is_speaking: true,
                confidence: 0.96,
            },
            NvidiaPerFrameSpeaker {
                timestamp: 0.12,
                speaker_bbox: Some([0.75, 0.22, 0.19, 0.25]),
                diarized_speaker_id: Some("S2".to_string()),
                face_id: Some(2),
                is_speaking: true,
                confidence: 0.91,
            },
        ];

        let timeline = aggregate_nvidia_frames_to_timeline(&raw_frames, 10.0, 30.0);
        assert!(!timeline.segments.is_empty());
        assert_eq!(timeline.speaker_person_mapping.get("S1"), Some(&1));
        assert_eq!(timeline.speaker_person_mapping.get("S2"), Some(&2));
        assert_eq!(timeline.provider, "nvidia_api");
    }

    #[test]
    fn test_conversion_layer_pipeline_end_to_end() {
        let json_response = r#"{
            "frames": [
                {
                    "timestamp": 0.04,
                    "faces": [
                        {
                            "face_id": 1,
                            "bbox": [0.20, 0.15, 0.15, 0.20],
                            "is_speaking": true,
                            "confidence": 0.95,
                            "diarized_speaker_id": "SPEAKER_00"
                        },
                        {
                            "face_id": 2,
                            "bbox": [0.70, 0.18, 0.14, 0.22],
                            "is_speaking": false,
                            "confidence": 0.10,
                            "diarized_speaker_id": null
                        }
                    ]
                },
                {
                    "timestamp": 0.08,
                    "faces": [
                        {
                            "face_id": 1,
                            "bbox": [0.20, 0.15, 0.15, 0.20],
                            "is_speaking": true,
                            "confidence": 0.93,
                            "diarized_speaker_id": "SPEAKER_00"
                        }
                    ]
                },
                {
                    "timestamp": 0.12,
                    "faces": [
                        {
                            "face_id": 1,
                            "bbox": [0.20, 0.15, 0.15, 0.20],
                            "is_speaking": true,
                            "confidence": 0.96,
                            "diarized_speaker_id": "SPEAKER_00"
                        }
                    ]
                },
                {
                    "timestamp": 0.16,
                    "faces": [
                        {
                            "face_id": 1,
                            "bbox": [0.20, 0.15, 0.15, 0.20],
                            "is_speaking": true,
                            "confidence": 0.94,
                            "diarized_speaker_id": "SPEAKER_00"
                        }
                    ]
                },
                {
                    "timestamp": 0.20,
                    "faces": [
                        {
                            "face_id": 1,
                            "bbox": [0.20, 0.15, 0.15, 0.20],
                            "is_speaking": true,
                            "confidence": 0.95,
                            "diarized_speaker_id": "SPEAKER_00"
                        }
                    ]
                }
            ]
        }"#;

        let parsed_frames = parse_nvidia_asd_response(json_response).expect("must parse NIM json");
        assert_eq!(parsed_frames.len(), 5);

        // Run full conversion pipeline
        let timeline = convert_nvidia_frames_to_timeline(&parsed_frames, 0.0, 10.0, None);
        assert!(!timeline.segments.is_empty());
        assert_eq!(timeline.speaker_person_mapping.get("SPEAKER_00"), Some(&1));
        assert_eq!(timeline.segments[0].person_id, 1);
        assert!(timeline.segments[0].confidence > 0.90);
    }

    #[test]
    fn test_temporal_smoothing_bridges_speech_gaps_and_prunes_spikes() {
        let detections = vec![
            // A quick 0.04s noise spike by Person 2
            AssociatedFrameDetection {
                timestamp_sec: 1.0,
                absolute_timestamp_sec: 1.0,
                person_id: 2,
                speaker_bbox: None,
                bbox_center: None,
                diarized_speaker_id: None,
                is_speaking: true,
                confidence: 0.70,
            },
            // Person 1 speaks from 2.0 to 2.2, pauses 0.2s, speaks 2.4 to 2.8 (should bridge)
            AssociatedFrameDetection {
                timestamp_sec: 2.0,
                absolute_timestamp_sec: 2.0,
                person_id: 1,
                speaker_bbox: None,
                bbox_center: None,
                diarized_speaker_id: Some("S1".to_string()),
                is_speaking: true,
                confidence: 0.92,
            },
            AssociatedFrameDetection {
                timestamp_sec: 2.1,
                absolute_timestamp_sec: 2.1,
                person_id: 1,
                speaker_bbox: None,
                bbox_center: None,
                diarized_speaker_id: Some("S1".to_string()),
                is_speaking: true,
                confidence: 0.95,
            },
            AssociatedFrameDetection {
                timestamp_sec: 2.4,
                absolute_timestamp_sec: 2.4,
                person_id: 1,
                speaker_bbox: None,
                bbox_center: None,
                diarized_speaker_id: Some("S1".to_string()),
                is_speaking: true,
                confidence: 0.93,
            },
            AssociatedFrameDetection {
                timestamp_sec: 2.6,
                absolute_timestamp_sec: 2.6,
                person_id: 1,
                speaker_bbox: None,
                bbox_center: None,
                diarized_speaker_id: Some("S1".to_string()),
                is_speaking: true,
                confidence: 0.91,
            },
        ];

        let segments = smooth_and_build_active_speaker_segments(&detections, 0.0, 10.0);
        // Spurious spike (0.05s duration) by Person 2 should be pruned
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].person_id, 1);
        // The pause between 2.1 and 2.4 (0.3s) was bridged into a continuous segment
        assert!(segments[0].end >= 2.6);
    }

    #[test]
    fn test_bounding_box_and_coordinate_normalization() {
        // Test standard [x, y, w, h]
        let raw_box = [0.10, 0.15, 0.30, 0.40];
        let (bbox, center) = normalize_bounding_box(&raw_box);
        let b = bbox.unwrap();
        assert_eq!(b[0], 0.10);
        assert_eq!(b[1], 0.15);
        assert!((b[2] - 0.30).abs() < 1e-4);
        assert!((b[3] - 0.40).abs() < 1e-4);
        let c = center.unwrap();
        assert!((c.0 - 0.25).abs() < 1e-4);
        assert!((c.1 - 0.35).abs() < 1e-4);

        // Test corners [x1, y1, x2, y2]
        let raw_corners = [0.60, 0.50, 0.90, 0.85];
        let (c_bbox, _) = normalize_bounding_box(&raw_corners);
        let cb = c_bbox.unwrap();
        assert_eq!(cb[0], 0.60);
        assert_eq!(cb[1], 0.50);
        assert!((cb[2] - 0.30).abs() < 1e-4);
        assert!((cb[3] - 0.35).abs() < 1e-4);

        // Test pixel normalization
        let raw_pixels = [192.0, 108.0, 576.0, 432.0];
        let (px_bbox, _) = normalize_bounding_box(&raw_pixels);
        assert!(px_bbox.is_some());
        let pb = px_bbox.unwrap();
        assert!(pb[0] <= 1.0);
        assert!(pb[1] <= 1.0);
    }

    #[test]
    fn test_associate_faces_and_persons_with_vision_keyframes() {
        use crate::media::face_tracker::{VisionKeyframe, VisionPersonTrack, VisionTrackingPayload};

        let face_tracker_result = FaceTrackerResult {
            avg_center_x: 0.50,
            face_detected: true,
            width: Some(1920.0),
            height: Some(1080.0),
            podcast: Some(VisionTrackingPayload {
                two_faces_detected: true,
                top_center_x: None,
                top_center_y: None,
                bottom_center_x: None,
                bottom_center_y: None,
                people: vec![
                    VisionPersonTrack {
                        id: 1,
                        name: "Speaker Left".to_string(),
                        keyframes: vec![VisionKeyframe {
                            t: 5.0,
                            x: 0.25,
                            y: 0.35,
                            width: 0.15,
                            height: 0.20,
                            confidence: 1.0,
                            visible: true,
                            state: None,
                        }],
                    },
                    VisionPersonTrack {
                        id: 2,
                        name: "Speaker Right".to_string(),
                        keyframes: vec![VisionKeyframe {
                            t: 5.0,
                            x: 0.75,
                            y: 0.35,
                            width: 0.15,
                            height: 0.20,
                            confidence: 1.0,
                            visible: true,
                            state: None,
                        }],
                    },
                ],
            }),
        };

        let detections = vec![
            NormalizedFrameDetection {
                timestamp_sec: 5.0,
                absolute_timestamp_sec: 5.0,
                speaker_bbox: Some([0.20, 0.25, 0.15, 0.20]),
                bbox_center: Some((0.275, 0.35)),
                raw_face_id: None,
                diarized_speaker_id: Some("SPK_LEFT".to_string()),
                is_speaking: true,
                confidence: 0.94,
            },
            NormalizedFrameDetection {
                timestamp_sec: 5.0,
                absolute_timestamp_sec: 5.0,
                speaker_bbox: Some([0.70, 0.25, 0.15, 0.20]),
                bbox_center: Some((0.775, 0.35)),
                raw_face_id: None,
                diarized_speaker_id: Some("SPK_RIGHT".to_string()),
                is_speaking: true,
                confidence: 0.92,
            },
        ];

        let associated = associate_faces_and_persons(&detections, Some(&face_tracker_result));
        assert_eq!(associated.detections.len(), 2);
        assert_eq!(associated.detections[0].person_id, 1);
        assert_eq!(associated.detections[1].person_id, 2);
        assert_eq!(associated.speaker_person_mapping.get("SPK_LEFT"), Some(&1));
        assert_eq!(associated.speaker_person_mapping.get("SPK_RIGHT"), Some(&2));
    }

    #[test]
    fn test_generate_speaker_aware_keyframes_hysteresis() {
        let timeline = ActiveSpeakerTimeline {
            segments: vec![
                ActiveSpeakerSegment {
                    start: 10.0,
                    end: 14.0,
                    person_id: 1,
                    confidence: 0.95,
                    speaker_label: Some("S1".to_string()),
                },
                // Rapid 0.8s interruption by Person 2 - should be stabilized by hysteresis
                ActiveSpeakerSegment {
                    start: 14.0,
                    end: 14.8,
                    person_id: 2,
                    confidence: 0.85,
                    speaker_label: Some("S2".to_string()),
                },
                ActiveSpeakerSegment {
                    start: 14.8,
                    end: 20.0,
                    person_id: 1,
                    confidence: 0.95,
                    speaker_label: Some("S1".to_string()),
                },
            ],
            provider: "local_vision_fusion".to_string(),
            source_duration: 30.0,
            speaker_person_mapping: HashMap::new(),
            confidence: 0.90,
            fallback_reason: None,
        };

        let face_result = FaceTrackerResult {
            avg_center_x: 0.50,
            face_detected: true,
            width: Some(1920.0),
            height: Some(1080.0),
            podcast: None,
        };

        let keyframes = generate_speaker_aware_keyframes(&timeline, &face_result, 10.0, 20.0);
        assert!(!keyframes.is_empty());
        assert_eq!(keyframes[0].t, 0.0);
    }

    #[test]
    fn test_visual_validation_scores() {
        let timeline = ActiveSpeakerTimeline::default();
        let face_result = FaceTrackerResult {
            avg_center_x: 0.50,
            face_detected: true,
            width: Some(1920.0),
            height: Some(0.25),
            podcast: None,
        };

        let validation = validate_clip_visuals(&timeline, &face_result, 0.0, 15.0);
        assert!(validation.face_detected);
        assert!(validation.visual_score > 0.0);
    }

    #[test]
    fn test_temporal_evidence_mapping_immune_to_speaker_duration() {
        use crate::media::face_tracker::{VisionKeyframe, VisionPersonTrack};
        use crate::models::{TranscriptSegment, TranscriptWord};

        // Scenario from user:
        // Person 1 speaks for 10 seconds (S1) at [0.0..10.0]
        // Person 2 speaks for 40 seconds (S2) at [10.0..50.0]
        // In the old broken heuristic, S2 was sorted first and wrongly assigned to Person 1!
        // In the new temporal evidence detector, temporal visibility establishes:
        // - During [0..10], Person 1 is on screen, Person 2 is absent -> S1 is Person 1.
        // - During [10..50], Person 2 is on screen, Person 1 is absent -> S2 is Person 2.
        let people = vec![
            VisionPersonTrack {
                id: 1,
                name: "Person 1".to_string(),
                keyframes: vec![
                    VisionKeyframe {
                        t: 2.0,
                        x: 0.30,
                        y: 0.35,
                        width: 0.15,
                        height: 0.20,
                        confidence: 1.0,
                        visible: true,
                        state: None,
                    },
                    VisionKeyframe {
                        t: 6.0,
                        x: 0.30,
                        y: 0.35,
                        width: 0.15,
                        height: 0.20,
                        confidence: 1.0,
                        visible: true,
                        state: None,
                    },
                    // Person 1 exits after 10.0
                    VisionKeyframe {
                        t: 25.0,
                        x: 0.30,
                        y: 0.35,
                        width: 0.15,
                        height: 0.20,
                        confidence: 0.0,
                        visible: false,
                        state: Some("exited".to_string()),
                    },
                ],
            },
            VisionPersonTrack {
                id: 2,
                name: "Person 2".to_string(),
                keyframes: vec![
                    // Person 2 absent before 10.0
                    VisionKeyframe {
                        t: 5.0,
                        x: 0.70,
                        y: 0.35,
                        width: 0.15,
                        height: 0.20,
                        confidence: 0.0,
                        visible: false,
                        state: Some("tentative".to_string()),
                    },
                    VisionKeyframe {
                        t: 15.0,
                        x: 0.70,
                        y: 0.35,
                        width: 0.15,
                        height: 0.20,
                        confidence: 1.0,
                        visible: true,
                        state: None,
                    },
                    VisionKeyframe {
                        t: 35.0,
                        x: 0.70,
                        y: 0.35,
                        width: 0.15,
                        height: 0.20,
                        confidence: 1.0,
                        visible: true,
                        state: None,
                    },
                ],
            },
        ];

        let transcript = NormalizedTranscript {
            language: "en".to_string(),
            duration: 50.0,
            speakers: vec!["S1".to_string(), "S2".to_string()],
            words: vec![
                TranscriptWord {
                    text: "S1".to_string(),
                    start: 0.0,
                    end: 10.0,
                    speaker: Some("S1".to_string()),
                },
                TranscriptWord {
                    text: "S2".to_string(),
                    start: 10.0,
                    end: 50.0,
                    speaker: Some("S2".to_string()),
                },
            ],
            segments: vec![
                TranscriptSegment {
                    start: 0.0,
                    end: 10.0,
                    text: "S1 short turn.".to_string(),
                    speaker: Some("S1".to_string()),
                },
                TranscriptSegment {
                    start: 10.0,
                    end: 50.0,
                    text: "S2 very long turn speaking for 40 seconds.".to_string(),
                    speaker: Some("S2".to_string()),
                },
            ],
        };

        let (mapping, confidence) = compute_temporal_speaker_person_mapping(
            "dummy_video.mp4",
            0.0,
            50.0,
            &transcript,
            &people,
        );

        // Verification: Despite S2 talking for 4x longer than S1 (40s vs 10s),
        // S1 is correctly mapped to Person 1 and S2 to Person 2!
        assert_eq!(mapping.get("S1"), Some(&1));
        assert_eq!(mapping.get("S2"), Some(&2));
        assert!(confidence >= 0.85);
    }

    #[test]
    fn test_compute_bounding_box_iou_precision() {
        let b1 = [0.10, 0.10, 0.20, 0.20]; // area = 0.04
        let b2 = [0.10, 0.10, 0.20, 0.20]; // identical
        assert!((compute_bounding_box_iou(&b1, &b2) - 1.0).abs() < 1e-4);

        let b3 = [0.50, 0.50, 0.20, 0.20]; // disjoint
        assert_eq!(compute_bounding_box_iou(&b1, &b3), 0.0);

        let b4 = [0.20, 0.10, 0.20, 0.20]; // 50% horizontal overlap
        // inter_w = 0.10, inter_h = 0.20, inter = 0.02
        // area1 = 0.04, area2 = 0.04, union = 0.08 - 0.02 = 0.06
        // iou = 0.02 / 0.06 = 1/3
        let iou = compute_bounding_box_iou(&b1, &b4);
        assert!((iou - (1.0 / 3.0)).abs() < 1e-4);
    }

    #[test]
    fn test_nvidia_face_id_to_clipon_persontrack_fusion() {
        use crate::media::face_tracker::{VisionKeyframe, VisionPersonTrack, VisionTrackingPayload};

        // ClipOn internal PersonTracks:
        // Person 1 (left): x = 0.26, y = 0.35, width = 0.18, height = 0.22
        // Person 2 (right): x = 0.74, y = 0.35, width = 0.18, height = 0.22
        let face_tracker_result = FaceTrackerResult {
            avg_center_x: 0.50,
            face_detected: true,
            width: Some(1920.0),
            height: Some(1080.0),
            podcast: Some(VisionTrackingPayload {
                two_faces_detected: true,
                top_center_x: None,
                top_center_y: None,
                bottom_center_x: None,
                bottom_center_y: None,
                people: vec![
                    VisionPersonTrack {
                        id: 1,
                        name: "Person 1 (Left)".to_string(),
                        keyframes: vec![
                            VisionKeyframe {
                                t: 0.0,
                                x: 0.26,
                                y: 0.35,
                                width: 0.18,
                                height: 0.22,
                                confidence: 1.0,
                                visible: true,
                                state: None,
                            },
                            VisionKeyframe {
                                t: 2.0,
                                x: 0.26,
                                y: 0.35,
                                width: 0.18,
                                height: 0.22,
                                confidence: 1.0,
                                visible: true,
                                state: None,
                            },
                        ],
                    },
                    VisionPersonTrack {
                        id: 2,
                        name: "Person 2 (Right)".to_string(),
                        keyframes: vec![
                            VisionKeyframe {
                                t: 0.0,
                                x: 0.74,
                                y: 0.35,
                                width: 0.18,
                                height: 0.22,
                                confidence: 1.0,
                                visible: true,
                                state: None,
                            },
                            VisionKeyframe {
                                t: 2.0,
                                x: 0.74,
                                y: 0.35,
                                width: 0.18,
                                height: 0.22,
                                confidence: 1.0,
                                visible: true,
                                state: None,
                            },
                        ],
                    },
                ],
            }),
        };

        // NVIDIA ASD NIM emits arbitrary face IDs:
        // face_id = 888 (located on left: bbox [0.18, 0.24, 0.16, 0.22], center 0.26)
        // face_id = 999 (located on right: bbox [0.66, 0.24, 0.16, 0.22], center 0.74)
        let detections = vec![
            NormalizedFrameDetection {
                timestamp_sec: 0.5,
                absolute_timestamp_sec: 0.5,
                speaker_bbox: Some([0.18, 0.24, 0.16, 0.22]),
                bbox_center: Some((0.26, 0.35)),
                raw_face_id: Some(888),
                diarized_speaker_id: Some("SPK_LEFT".to_string()),
                is_speaking: true,
                confidence: 0.95,
            },
            NormalizedFrameDetection {
                timestamp_sec: 0.5,
                absolute_timestamp_sec: 0.5,
                speaker_bbox: Some([0.66, 0.24, 0.16, 0.22]),
                bbox_center: Some((0.74, 0.35)),
                raw_face_id: Some(999),
                diarized_speaker_id: Some("SPK_RIGHT".to_string()),
                is_speaking: false,
                confidence: 0.93,
            },
            NormalizedFrameDetection {
                timestamp_sec: 1.0,
                absolute_timestamp_sec: 1.0,
                speaker_bbox: Some([0.18, 0.24, 0.16, 0.22]),
                bbox_center: Some((0.26, 0.35)),
                raw_face_id: Some(888),
                diarized_speaker_id: Some("SPK_LEFT".to_string()),
                is_speaking: true,
                confidence: 0.96,
            },
            NormalizedFrameDetection {
                timestamp_sec: 1.5,
                absolute_timestamp_sec: 1.5,
                speaker_bbox: Some([0.66, 0.24, 0.16, 0.22]),
                bbox_center: Some((0.74, 0.35)),
                raw_face_id: Some(999),
                diarized_speaker_id: Some("SPK_RIGHT".to_string()),
                is_speaking: true,
                confidence: 0.94,
            },
        ];

        // 1. Run Bipartite Fusion Layer
        let associated = associate_faces_and_persons(&detections, Some(&face_tracker_result));
        assert_eq!(associated.detections.len(), 4);

        // Verification: NVIDIA face_id 888 is explicitly mapped to ClipOn Person 1!
        assert_eq!(associated.detections[0].person_id, 1);
        assert_eq!(associated.detections[2].person_id, 1);

        // Verification: NVIDIA face_id 999 is explicitly mapped to ClipOn Person 2!
        assert_eq!(associated.detections[1].person_id, 2);
        assert_eq!(associated.detections[3].person_id, 2);

        // Audio diarization mapping is also aligned
        assert_eq!(associated.speaker_person_mapping.get("SPK_LEFT"), Some(&1));
        assert_eq!(associated.speaker_person_mapping.get("SPK_RIGHT"), Some(&2));

        // 2. Build segments and verify end-to-end timeline conversion
        let segments = smooth_and_build_active_speaker_segments(
            &associated.detections,
            0.0,
            2.0,
        );
        assert!(!segments.is_empty());
        assert_eq!(segments[0].person_id, 1);

        // 3. Verify keyframe generation accurately targets Person 1 and Person 2
        let timeline = ActiveSpeakerTimeline {
            segments: vec![
                ActiveSpeakerSegment {
                    start: 0.0,
                    end: 3.0,
                    person_id: 1, // Mapped from NVIDIA 888
                    confidence: 0.95,
                    speaker_label: Some("SPK_LEFT".to_string()),
                },
                ActiveSpeakerSegment {
                    start: 3.0,
                    end: 6.0,
                    person_id: 2, // Mapped from NVIDIA 999
                    confidence: 0.94,
                    speaker_label: Some("SPK_RIGHT".to_string()),
                },
            ],
            provider: "nvidia_api".to_string(),
            source_duration: 6.0,
            speaker_person_mapping: associated.speaker_person_mapping,
            confidence: 0.95,
            fallback_reason: None,
        };

        let keyframes = generate_speaker_aware_keyframes(&timeline, &face_tracker_result, 0.0, 6.0);
        assert!(!keyframes.is_empty());
        // First keyframe centers on Person 1 (x ~ 0.26)
        assert!((keyframes[0].x - 0.26).abs() < 0.05);
        // Final keyframe centers on Person 2 (x ~ 0.74)
        let last_kf = keyframes.last().unwrap();
        assert!((last_kf.x - 0.74).abs() < 0.05);
    }

    #[test]
    fn test_compute_active_speaker_cache_key_invalidation() {
        let key1 = compute_active_speaker_cache_key("video.mp4", 0.0, 10.0);
        let key2 = compute_active_speaker_cache_key("video.mp4", 0.0, 10.0);
        assert_eq!(key1, key2, "Cache key must be deterministic for identical parameters");

        let key_diff_dur = compute_active_speaker_cache_key("video.mp4", 0.0, 15.0);
        assert_ne!(key1, key_diff_dur, "Cache key must change when duration changes");

        let key_diff_source = compute_active_speaker_cache_key("other_video.mp4", 0.0, 10.0);
        assert_ne!(key1, key_diff_source, "Cache key must change when source changes");

        // Verify configuration components are included in the key hash
        let analyzer_str = format!("{}_{}", ASD_ANALYSIS_VERSION, ASD_FACE_TRACKER_VERSION);
        let model_str = format!("nim_{}", get_nvidia_asd_model_identifier());
        let params_str = format!(
            "thresh_{:.2}_hold_{:.1}_diar_{}",
            ASD_SPEAKING_THRESHOLD, ASD_MIN_HOLD_SEC, ASD_DIARIZATION_VERSION
        );

        let direct_key = AnalysisCache::compute_source_key_with_params(
            "video.mp4",
            0.0,
            10.0,
            &analyzer_str,
            &model_str,
            &params_str,
        );
        assert_eq!(key1, direct_key);

        // Verify that changing any of the 7 parameters invalidates the key
        let diff_analyzer = AnalysisCache::compute_source_key_with_params(
            "video.mp4",
            0.0,
            10.0,
            "asd_fusion_v5_apple_vision_v2", // changed analysis version
            &model_str,
            &params_str,
        );
        assert_ne!(key1, diff_analyzer);

        let diff_face = AnalysisCache::compute_source_key_with_params(
            "video.mp4",
            0.0,
            10.0,
            "asd_fusion_v4_apple_vision_v3", // changed face tracker version
            &model_str,
            &params_str,
        );
        assert_ne!(key1, diff_face);

        let diff_model = AnalysisCache::compute_source_key_with_params(
            "video.mp4",
            0.0,
            10.0,
            &analyzer_str,
            "nim_custom_func_123", // changed NVIDIA model/function
            &params_str,
        );
        assert_ne!(key1, diff_model);

        let diff_thresh = AnalysisCache::compute_source_key_with_params(
            "video.mp4",
            0.0,
            10.0,
            &analyzer_str,
            &model_str,
            "thresh_0.65_hold_2.0_diar_diarize_v1", // changed threshold
        );
        assert_ne!(key1, diff_thresh);

        let diff_diar = AnalysisCache::compute_source_key_with_params(
            "video.mp4",
            0.0,
            10.0,
            &analyzer_str,
            &model_str,
            "thresh_0.55_hold_2.0_diar_diarize_v2", // changed diarization version
        );
        assert_ne!(key1, diff_diar);
    }
}
