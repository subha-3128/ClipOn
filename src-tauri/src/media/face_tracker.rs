use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionKeyframe {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default = "default_one")]
    pub confidence: f64,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub state: Option<String>,
}

fn default_one() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionPersonTrack {
    pub id: usize,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub keyframes: Vec<VisionKeyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayoutSegment {
    pub start: f64,
    pub end: f64,
    pub number_of_people: usize,
    pub layout_type: String, // "single", "split_two", "split_three"
    pub person_ids: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisionTrackingPayload {
    #[serde(default)]
    pub two_faces_detected: bool,
    #[serde(default)]
    pub top_center_x: Option<f64>,
    #[serde(default)]
    pub top_center_y: Option<f64>,
    #[serde(default)]
    pub bottom_center_x: Option<f64>,
    #[serde(default)]
    pub bottom_center_y: Option<f64>,
    #[serde(default)]
    pub people: Vec<VisionPersonTrack>,
    #[serde(default)]
    pub segments: Vec<LayoutSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceTrackerResult {
    pub avg_center_x: f64,
    pub face_detected: bool,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default, alias = "podcast")]
    pub tracking: Option<VisionTrackingPayload>,
}

impl FaceTrackerResult {
    pub fn people(&self) -> &[VisionPersonTrack] {
        self.tracking
            .as_ref()
            .map(|p| p.people.as_slice())
            .unwrap_or(&[])
    }

    pub fn segments(&self) -> &[LayoutSegment] {
        self.tracking
            .as_ref()
            .map(|p| p.segments.as_slice())
            .unwrap_or(&[])
    }
}

pub fn merge_adjacent_segments(segments: &[LayoutSegment]) -> Vec<LayoutSegment> {
    if segments.is_empty() {
        return Vec::new();
    }
    let mut merged: Vec<LayoutSegment> = vec![segments[0].clone()];
    for seg in &segments[1..] {
        let last = merged.last_mut().unwrap();
        if last.layout_type == seg.layout_type && last.person_ids == seg.person_ids {
            last.end = seg.end;
        } else {
            merged.push(seg.clone());
        }
    }
    merged
}

impl Default for FaceTrackerResult {
    fn default() -> Self {
        Self {
            avg_center_x: 0.5,
            face_detected: false,
            width: None,
            height: None,
            tracking: None,
        }
    }
}

pub struct FaceTracker;


impl FaceTracker {
    /// Resolves the face tracker binary dynamically with zero hardcoded developer paths.
    pub fn resolve_tracker_binary() -> Result<PathBuf> {
        #[cfg(not(target_os = "macos"))]
        {
            return Err(anyhow!(
                "Face tracking requires Apple Vision framework and is currently supported on macOS only"
            ));
        }

        // 1. Current executable directory (when running packaged application or cargo test/run)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let candidates = [
                    parent.join("clipon-face-tracker"),
                    parent.join("../Resources/bin/clipon-face-tracker"),
                    parent.join("bin/clipon-face-tracker"),
                    parent.join("../../../bin/clipon-face-tracker"),
                    parent.join("../../../src-tauri/bin/clipon-face-tracker"),
                ];
                for c in &candidates {
                    if c.exists() {
                        return Ok(c.clone());
                    }
                }
            }
        }

        // 2. Standard macOS application installation directory
        let installed_app_candidates = [
            PathBuf::from("/Applications/ClipOn.app/Contents/MacOS/clipon-face-tracker"),
            PathBuf::from("/Applications/ClipOn.app/Contents/Resources/bin/clipon-face-tracker"),
        ];
        for c in &installed_app_candidates {
            if c.exists() {
                return Ok(c.clone());
            }
        }

        // 3. Current working directory relative paths
        let cwd_candidates = [
            PathBuf::from("src-tauri/bin/clipon-face-tracker"),
            PathBuf::from("bin/clipon-face-tracker"),
        ];
        for c in &cwd_candidates {
            if c.exists() {
                return Ok(c.clone());
            }
        }

        // 4. Standard system binary directories
        let sys_candidates = [
            PathBuf::from("/opt/homebrew/bin/clipon-face-tracker"),
            PathBuf::from("/usr/local/bin/clipon-face-tracker"),
            PathBuf::from("/usr/bin/clipon-face-tracker"),
        ];
        for c in &sys_candidates {
            if c.exists() {
                return Ok(c.clone());
            }
        }

        Err(anyhow!("clipon-face-tracker executable not found"))
    }

    /// Analyzes the video clip interval and detects face center positioning.
    pub fn analyze(
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
    ) -> FaceTrackerResult {
        let cache = crate::analysis_cache::AnalysisCache::global();
        let cache_key = crate::analysis_cache::AnalysisCache::compute_source_key_with_params(
            source_path,
            start_sec,
            duration_sec,
            "face_tracker_v2",
            "vision_landmarks",
            "center_x_tracking",
        );

        if let Some(cached) = cache.get::<FaceTrackerResult>(&cache_key, "face_tracking") {
            return cached;
        }
        if let Some(cached) = cache.get::<FaceTrackerResult>(&cache_key, "podcast_analysis") {
            return cached;
        }

        let binary_path = match Self::resolve_tracker_binary() {
            Ok(p) => p,
            Err(_) => return FaceTrackerResult::default(),
        };

        let mut cmd = Command::new(&binary_path);
        cmd.arg(source_path);
        cmd.arg(format!("{start_sec:.3}"));
        cmd.arg(format!("{duration_sec:.3}"));

        let output = match cmd.output() {
            Ok(o) if o.status.success() => o,
            _ => return FaceTrackerResult::default(),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let res = serde_json::from_str::<FaceTrackerResult>(stdout.trim())
            .unwrap_or_default();

        if res.face_detected || res.tracking.is_some() {
            let _ = cache.put(&cache_key, "face_tracking", &res);
        }

        res
    }

    /// Detect face horizontal center X in normalized coords [0.0, 1.0] using Apple Vision.
    pub fn detect_face_center_x(source_path: &str, start_sec: f64, duration_sec: f64) -> f64 {
        Self::analyze(source_path, start_sec, duration_sec)
            .avg_center_x
            .clamp(0.20, 0.80)
    }
}

pub fn detect_faces_full(source_path: &str, start_sec: f64, duration_sec: f64) -> FaceTrackerResult {
    FaceTracker::analyze(source_path, start_sec, duration_sec)
}

pub fn detect_face_center_x(source_path: &str, start_sec: f64, duration_sec: f64) -> f64 {
    FaceTracker::detect_face_center_x(source_path, start_sec, duration_sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_tracker_binary() {
        let res = FaceTracker::resolve_tracker_binary();
        assert!(
            res.is_ok(),
            "Tracker binary should resolve: {:?}",
            res.err()
        );
    }

    #[test]
    fn test_merge_adjacent_segments() {
        let segs = vec![
            LayoutSegment {
                start: 0.0,
                end: 3.0,
                number_of_people: 2,
                layout_type: "split_two".to_string(),
                person_ids: vec![1, 2],
            },
            LayoutSegment {
                start: 3.0,
                end: 6.0,
                number_of_people: 2,
                layout_type: "split_two".to_string(),
                person_ids: vec![1, 2],
            },
            LayoutSegment {
                start: 6.0,
                end: 10.0,
                number_of_people: 1,
                layout_type: "single".to_string(),
                person_ids: vec![1],
            },
        ];

        let merged = merge_adjacent_segments(&segs);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start, 0.0);
        assert_eq!(merged[0].end, 6.0);
        assert_eq!(merged[0].layout_type, "split_two");
        assert_eq!(merged[1].start, 6.0);
        assert_eq!(merged[1].end, 10.0);
        assert_eq!(merged[1].layout_type, "single");
    }
}
