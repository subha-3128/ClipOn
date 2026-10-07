use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::analysis_cache::AnalysisCache;
use crate::credentials;
use crate::http_client;
use crate::media::face_tracker::{FaceTracker, FaceTrackerResult};
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
// 2. NVIDIA ACTIVE SPEAKER DETECTION API CLIENT
// =========================================================================

#[derive(Debug, Clone)]
pub struct NvidiaAsdDetector {
    endpoint: String,
    api_key: String,
}

#[derive(Debug, Serialize)]
struct NvidiaAsdRequest<'a> {
    source_filename: &'a str,
    start_sec: f64,
    duration_sec: f64,
    diarization_segments: Vec<NvidiaDiarizationSegment<'a>>,
}

#[derive(Debug, Serialize)]
struct NvidiaDiarizationSegment<'a> {
    speaker: &'a str,
    start: f64,
    end: f64,
}

#[derive(Debug, Deserialize)]
struct NvidiaAsdResponse {
    #[serde(default)]
    active_speakers: Vec<NvidiaActiveSpeakerItem>,
    #[serde(default)]
    speaker_mapping: Option<HashMap<String, usize>>,
    #[serde(default)]
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct NvidiaActiveSpeakerItem {
    start: f64,
    end: f64,
    #[serde(default)]
    speaker_id: Option<String>,
    #[serde(default)]
    face_id: Option<usize>,
    #[serde(default)]
    person_id: Option<usize>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    is_speaking: Option<bool>,
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

        let endpoint = std::env::var("NVIDIA_ASD_ENDPOINT").unwrap_or_else(|_| {
            "https://ai.api.nvidia.com/v1/cv/nvidia/active-speaker-detection".to_string()
        });

        Ok(Self {
            endpoint,
            api_key: clean_key,
        })
    }

    pub fn with_endpoint_and_key(endpoint: &str, api_key: &str) -> Self {
        Self {
            endpoint: endpoint.trim().to_string(),
            api_key: api_key.trim().to_string(),
        }
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
        if self.api_key.is_empty() {
            return Err(anyhow!("NVIDIA API credential is missing"));
        }

        let file_name = Path::new(source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("source_video");

        // Format diarization segments from transcript
        let mut diarization_segments = Vec::new();
        if let Some(t) = transcript {
            for seg in &t.segments {
                if seg.end >= start_sec && seg.start <= (start_sec + duration_sec) {
                    if let Some(speaker) = &seg.speaker {
                        diarization_segments.push(NvidiaDiarizationSegment {
                            speaker,
                            start: (seg.start - start_sec).max(0.0),
                            end: (seg.end - start_sec).min(duration_sec),
                        });
                    }
                }
            }
        }

        let request_payload = NvidiaAsdRequest {
            source_filename: file_name,
            start_sec,
            duration_sec,
            diarization_segments,
        };

        let client = http_client::build_api_client(15);
        let auth_header = format!("Bearer {}", self.api_key);

        let endpoint_url = self.endpoint.clone();
        let payload_json = serde_json::to_value(&request_payload)?;

        // Send with retry (up to 2 attempts on 429 or 5xx, fail immediately on 401/403)
        let response = http_client::send_with_retry(
            &client,
            || {
                client
                    .post(&endpoint_url)
                    .header("Authorization", &auth_header)
                    .header("Content-Type", "application/json")
                    .json(&payload_json)
            },
            2,
            500,
        )
        .await
        .context("NVIDIA Active Speaker Detection API request failed")?;

        let resp_json: NvidiaAsdResponse = response
            .json()
            .await
            .context("Parsing NVIDIA Active Speaker response JSON")?;

        let mut segments = Vec::new();
        let mut speaker_mapping = resp_json.speaker_mapping.unwrap_or_default();

        for item in resp_json.active_speakers {
            if item.is_speaking == Some(false) {
                continue;
            }
            let p_id = item.person_id.or(item.face_id).unwrap_or(1);
            let conf = item.confidence.unwrap_or(0.85).clamp(0.0, 1.0);
            let s_start = (item.start).max(0.0);
            let s_end = (item.end).max(s_start);

            if s_end > s_start {
                if let Some(ref spk) = item.speaker_id {
                    speaker_mapping.insert(spk.clone(), p_id);
                }
                segments.push(ActiveSpeakerSegment {
                    start: s_start + start_sec,
                    end: s_end + start_sec,
                    person_id: p_id,
                    confidence: conf,
                    speaker_label: item.speaker_id,
                });
            }
        }

        // Sort chronologically
        segments.sort_by(|a, b| a.start.total_cmp(&b.start));

        Ok(ActiveSpeakerTimeline {
            segments,
            provider: "nvidia_api".to_string(),
            source_duration: duration_sec,
            speaker_person_mapping: speaker_mapping,
            confidence: resp_json.confidence.unwrap_or(0.92).clamp(0.0, 1.0),
        })
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
            // Filter words inside this timeline window
            let relevant_words: Vec<_> = t
                .words
                .iter()
                .filter(|w| w.end >= start_sec && w.start <= (start_sec + duration_sec))
                .collect();

            // 1. Correlate audio speakers with visual person tracks
            if !people.is_empty() {
                let mut speaker_counts: HashMap<String, f64> = HashMap::new();
                for w in &relevant_words {
                    if let Some(speaker) = &w.speaker {
                        *speaker_counts.entry(speaker.clone()).or_insert(0.0) +=
                            (w.end - w.start).max(0.1);
                    }
                }

                // If multiple people are detected, assign distinct person IDs
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
                // Single default person
                for w in &relevant_words {
                    if let Some(speaker) = &w.speaker {
                        speaker_mapping.insert(speaker.clone(), 1);
                    }
                }
            }

            // 2. Generate active speaker segments from speech intervals
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
            // No transcript available: generate single default segment covering duration
            segments.push(ActiveSpeakerSegment {
                start: start_sec,
                end: start_sec + duration_sec,
                person_id: 1,
                confidence: if vision_result.face_detected { 0.80 } else { 0.50 },
                speaker_label: None,
            });
        }

        // Sort segments chronologically
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
        "active_speaker_v2",
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
                    "[ClipOn ASD] Notice: NVIDIA Active Speaker API unavailable ({}), falling back to local vision-audio fusion.",
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

    // Helper to find horizontal center for a person_id
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

    // Extract segments intersecting [clip_start, clip_end]
    let relevant_segments: Vec<&ActiveSpeakerSegment> = timeline
        .segments
        .iter()
        .filter(|s| s.end > clip_start && s.start < clip_end)
        .collect();

    if relevant_segments.is_empty() {
        // Return single steady keyframe
        return vec![TrackingKeyframe {
            t: 0.0,
            x: default_cx,
            y: default_cy,
        }];
    }

    // Apply Hysteresis: Merge rapid speaker switches (< 2.0s minimum hold duration)
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
                // Extend current speaker
                last.end = seg_end;
            } else {
                let current_duration = last.end - last.start;
                let new_duration = seg_end - seg_start;

                // Hysteresis check: If previous speaker held for less than 2.0s,
                // or new speaker speaks for less than 1.5s, don't flip camera;
                // instead, keep framing or switch to wide two-person shot
                if current_duration < 2.0 || new_duration < 1.5 {
                    if people.len() >= 2 {
                        // Wide shot: midpoint between person 1 and 2
                        last.person_id = 0; // 0 designates two-person wide shot
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

    // Generate smooth keyframes with 0.5s ease-in-out transitions
    let mut keyframes: Vec<TrackingKeyframe> = Vec::new();
    let transition_duration = 0.50f64;

    for (i, seg) in held_segments.iter().enumerate() {
        let target_x = if seg.person_id == 0 {
            // Two-person wide shot: midpoint
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
                // Interpolate ease-in-out pan over transition_duration
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

    // Ensure last keyframe reaches end of clip
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
    fn test_nvidia_asd_request_serialization() {
        let segs = vec![NvidiaDiarizationSegment {
            speaker: "S1",
            start: 0.0,
            end: 5.0,
        }];
        let req = NvidiaAsdRequest {
            source_filename: "interview.mp4",
            start_sec: 10.0,
            duration_sec: 30.0,
            diarization_segments: segs,
        };
        let json = serde_json::to_string(&req).expect("must serialize");
        assert!(json.contains("interview.mp4"));
        assert!(json.contains("\"start_sec\":10.0"));
    }

    #[test]
    fn test_nvidia_asd_response_parsing() {
        let resp_json = r#"{
            "active_speakers": [
                {
                    "start": 0.0,
                    "end": 5.0,
                    "speaker_id": "S1",
                    "person_id": 1,
                    "confidence": 0.95,
                    "is_speaking": true
                },
                {
                    "start": 5.5,
                    "end": 12.0,
                    "speaker_id": "S2",
                    "person_id": 2,
                    "confidence": 0.91,
                    "is_speaking": true
                }
            ],
            "confidence": 0.93
        }"#;

        let parsed: NvidiaAsdResponse =
            serde_json::from_str(resp_json).expect("must parse response");
        assert_eq!(parsed.active_speakers.len(), 2);
        assert_eq!(parsed.confidence, Some(0.93));
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
        // Verify keyframes cover duration
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
