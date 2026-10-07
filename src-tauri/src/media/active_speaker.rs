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
}

impl Default for ActiveSpeakerTimeline {
    fn default() -> Self {
        Self {
            segments: Vec::new(),
            provider: "local_vision_fusion".to_string(),
            source_duration: 0.0,
            speaker_person_mapping: HashMap::new(),
            confidence: 0.5,
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

/// Official per-frame speaker structure emitted by NVIDIA Active Speaker Detection NIM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaPerFrameSpeaker {
    #[serde(default)]
    pub timestamp: f64,
    #[serde(default)]
    pub speaker_bbox: Option<[f64; 4]>, // [x, y, width, height] normalized
    #[serde(default)]
    pub diarized_speaker_id: Option<String>,
    #[serde(default)]
    pub face_id: Option<usize>,
    #[serde(default)]
    pub is_speaking: bool,
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NvidiaAsdResponse {
    #[serde(default)]
    pub frames: Vec<NvidiaPerFrameSpeaker>,
    #[serde(default)]
    pub status: Option<String>,
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

/// Aggregates NVIDIA per-frame active speaker records into structured continuous segments
pub fn aggregate_nvidia_frames_to_timeline(
    frames: &[NvidiaPerFrameSpeaker],
    start_sec: f64,
    duration_sec: f64,
) -> ActiveSpeakerTimeline {
    if frames.is_empty() {
        return ActiveSpeakerTimeline {
            segments: Vec::new(),
            provider: "nvidia_api".to_string(),
            source_duration: duration_sec,
            speaker_person_mapping: HashMap::new(),
            confidence: 0.5,
        };
    }

    // 1. Correlate diarized audio speaker IDs with visual face IDs
    let mut co_occurrence: HashMap<(String, usize), usize> = HashMap::new();
    for f in frames {
        if f.is_speaking {
            if let (Some(ref spk), Some(face_id)) = (&f.diarized_speaker_id, f.face_id) {
                *co_occurrence.entry((spk.clone(), face_id)).or_insert(0) += 1;
            }
        }
    }

    let mut speaker_mapping: HashMap<String, usize> = HashMap::new();
    let mut mapped_speakers: HashMap<String, (usize, usize)> = HashMap::new();
    for ((spk, face_id), count) in co_occurrence {
        let entry = mapped_speakers.entry(spk).or_insert((face_id, count));
        if count > entry.1 {
            *entry = (face_id, count);
        }
    }
    for (spk, (face_id, _)) in mapped_speakers {
        speaker_mapping.insert(spk, face_id);
    }

    // 2. Cluster active frames into contiguous segments
    let mut active_frames: Vec<&NvidiaPerFrameSpeaker> =
        frames.iter().filter(|f| f.is_speaking).collect();
    active_frames.sort_by(|a, b| a.timestamp.total_cmp(&b.timestamp));

    let mut segments: Vec<ActiveSpeakerSegment> = Vec::new();
    let max_frame_gap = 0.35f64;

    for frame in active_frames {
        let face_id = frame.face_id.unwrap_or(1);
        let conf = frame.confidence.clamp(0.0, 1.0);
        let frame_time = frame.timestamp + start_sec;

        if let Some(last) = segments.last_mut() {
            if last.person_id == face_id && (frame_time - last.end) <= max_frame_gap {
                last.end = frame_time + 0.05;
                last.confidence = (last.confidence + conf) * 0.5;
                if last.speaker_label.is_none() && frame.diarized_speaker_id.is_some() {
                    last.speaker_label = frame.diarized_speaker_id.clone();
                }
                continue;
            }
        }

        segments.push(ActiveSpeakerSegment {
            start: frame_time,
            end: frame_time + 0.05,
            person_id: face_id,
            confidence: conf,
            speaker_label: frame.diarized_speaker_id.clone(),
        });
    }

    let avg_confidence = if segments.is_empty() {
        0.80
    } else {
        segments.iter().map(|s| s.confidence).sum::<f64>() / segments.len() as f64
    };

    ActiveSpeakerTimeline {
        segments,
        provider: "nvidia_api".to_string(),
        source_duration: duration_sec,
        speaker_person_mapping: speaker_mapping,
        confidence: avg_confidence.clamp(0.0, 1.0),
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

        let asd_data: NvidiaAsdResponse = final_response
            .json()
            .await
            .context("Parsing NVIDIA ASD response JSON")?;

        let timeline = aggregate_nvidia_frames_to_timeline(&asd_data.frames, start_sec, duration_sec);
        Ok(timeline)
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
        let vision_result = FaceTracker::analyze(source_path, start_sec, duration_sec);
        let people = vision_result.people();

        let mut speaker_mapping: HashMap<String, usize> = HashMap::new();
        let mut segments: Vec<ActiveSpeakerSegment> = Vec::new();

        if let Some(t) = transcript {
            let relevant_words: Vec<_> = t
                .words
                .iter()
                .filter(|w| w.end >= start_sec && w.start <= (start_sec + duration_sec))
                .collect();

            if !people.is_empty() {
                let mut speaker_counts: HashMap<String, f64> = HashMap::new();
                for w in &relevant_words {
                    if let Some(speaker) = &w.speaker {
                        *speaker_counts.entry(speaker.clone()).or_insert(0.0) +=
                            (w.end - w.start).max(0.1);
                    }
                }

                let mut sorted_speakers: Vec<_> = speaker_counts.into_iter().collect();
                sorted_speakers.sort_by(|a, b| b.1.total_cmp(&a.1));

                for (idx, (spk, _)) in sorted_speakers.iter().enumerate() {
                    let assigned_person_id = if idx < people.len() {
                        people[idx].id
                    } else {
                        idx + 1
                    };
                    speaker_mapping.insert(spk.clone(), assigned_person_id);
                }
            } else {
                for w in &relevant_words {
                    if let Some(speaker) = &w.speaker {
                        speaker_mapping.insert(speaker.clone(), 1);
                    }
                }
            }

            for seg in &t.segments {
                if seg.end < start_sec || seg.start > (start_sec + duration_sec) {
                    continue;
                }
                let s_start = seg.start.max(start_sec);
                let s_end = seg.end.min(start_sec + duration_sec);
                if s_end <= s_start {
                    continue;
                }

                let (p_id, conf) = if let Some(speaker) = &seg.speaker {
                    let pid = speaker_mapping.get(speaker).copied().unwrap_or(1);
                    (pid, 0.88)
                } else {
                    (1, 0.65)
                };

                segments.push(ActiveSpeakerSegment {
                    start: s_start,
                    end: s_end,
                    person_id: p_id,
                    confidence: conf,
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

        segments.sort_by(|a, b| a.start.total_cmp(&b.start));

        Ok(ActiveSpeakerTimeline {
            segments,
            provider: "local_vision_fusion".to_string(),
            source_duration: duration_sec,
            speaker_person_mapping: speaker_mapping,
            confidence: if vision_result.face_detected { 0.85 } else { 0.60 },
        })
    }
}

// =========================================================================
// 4. PERSISTENT CACHE & UNIFIED PIPELINE DISPATCHER
// =========================================================================

pub async fn get_or_compute_active_speaker_timeline(
    source_path: &str,
    start_sec: f64,
    duration_sec: f64,
    transcript: Option<&NormalizedTranscript>,
) -> ActiveSpeakerTimeline {
    let cache = AnalysisCache::global();
    let cache_key = AnalysisCache::compute_source_key_with_params(
        source_path,
        start_sec,
        duration_sec,
        "active_speaker_v3",
        "asd_nim_fusion",
        "primary_nvidia",
    );

    // 1. Check persistent AnalysisCache
    if let Some(cached) = cache.get::<ActiveSpeakerTimeline>(&cache_key, "active_speaker") {
        return cached;
    }

    // 2. Attempt primary NVIDIA API Active Speaker Detection
    let timeline_res = if let Ok(nvidia_detector) = NvidiaAsdDetector::try_new() {
        match nvidia_detector
            .detect_active_speakers(source_path, start_sec, duration_sec, transcript)
            .await
        {
            Ok(timeline) => Some(timeline),
            Err(e) => {
                eprintln!(
                    "[ClipOn ASD] Notice: NVIDIA Active Speaker inference fallback ({}). Using local multimodal fusion.",
                    e
                );
                None
            }
        }
    } else {
        None
    };

    // 3. Fallback to Local Multimodal Vision-Audio Fusion
    let final_timeline = match timeline_res {
        Some(t) => t,
        None => {
            let local_detector = LocalFusionDetector::new();
            local_detector
                .detect_active_speakers(source_path, start_sec, duration_sec, transcript)
                .await
                .unwrap_or_default()
        }
    };

    // 4. Cache validated active-speaker timeline
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

    let get_person_center = |pid: usize| -> f64 {
        if let Some(person) = people.iter().find(|p| p.id == pid) {
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
        let target_x = if seg.person_id == 0 {
            let x1 = get_person_center(1);
            let x2 = get_person_center(2);
            ((x1 + x2) / 2.0).clamp(0.25, 0.75)
        } else {
            get_person_center(seg.person_id)
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
}
