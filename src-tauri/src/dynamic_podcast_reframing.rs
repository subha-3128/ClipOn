use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonKeyframe {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub confidence: Option<f64>,
    pub visible: bool,
    pub state: Option<String>,
}

impl PersonKeyframe {
    pub fn new(t: f64, x: f64, y: f64) -> Self {
        Self {
            t,
            x,
            y,
            width: Some(0.2),
            height: Some(0.2),
            confidence: Some(1.0),
            visible: true,
            state: Some("visible".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonTrack {
    pub id: usize,
    pub name: Option<String>,
    pub keyframes: Vec<PersonKeyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSegment {
    pub start: f64,
    pub end: f64,
    pub number_of_people: usize,
    pub layout_type: String, // "single", "split_two", "split_three"
    pub person_ids: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodcastFaceTracking {
    pub top_center_x: f64,
    pub top_center_y: f64,
    pub bottom_center_x: f64,
    pub bottom_center_y: f64,
    pub two_faces_detected: bool,
    pub people: Option<Vec<PersonTrack>>,
    pub segments: Option<Vec<LayoutSegment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPodcastReframingResult {
    pub avg_center_x: f64,
    pub face_detected: bool,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub podcast: Option<PodcastFaceTracking>,
}

/// The DynamicPodcastReframing engine coordinates continuous multi-person tracking,
/// persistent identity maintenance, and dynamic vertical layout reframing (1P, 2P, 3P).
pub struct DynamicPodcastReframing;

impl DynamicPodcastReframing {
    /// Resolves the face tracker binary dynamically with zero hardcoded developer paths.
    pub fn resolve_tracker_binary() -> Result<PathBuf> {
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

    /// Analyzes the video clip timeline and produces continuous tracking and layout segment metadata.
    pub fn analyze(
        source_path: &str,
        start_sec: f64,
        duration_sec: f64,
    ) -> DynamicPodcastReframingResult {
        let cache = crate::analysis_cache::AnalysisCache::global();
        let cache_key = crate::analysis_cache::AnalysisCache::compute_source_key(
            source_path,
            start_sec,
            duration_sec,
        );

        if let Some(cached) =
            cache.get::<DynamicPodcastReframingResult>(&cache_key, "podcast_analysis")
        {
            return cached;
        }

        let binary_path = match Self::resolve_tracker_binary() {
            Ok(p) => p,
            Err(_) => {
                return DynamicPodcastReframingResult {
                    avg_center_x: 0.5,
                    face_detected: false,
                    width: None,
                    height: None,
                    podcast: None,
                };
            }
        };

        let mut cmd = Command::new(&binary_path);
        cmd.arg("-nostdin");
        cmd.arg(source_path);
        cmd.arg(format!("{start_sec:.3}"));
        cmd.arg(format!("{duration_sec:.3}"));

        let output = match cmd.output() {
            Ok(o) if o.status.success() => o,
            _ => {
                return DynamicPodcastReframingResult {
                    avg_center_x: 0.5,
                    face_detected: false,
                    width: None,
                    height: None,
                    podcast: None,
                };
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let res = serde_json::from_str::<DynamicPodcastReframingResult>(stdout.trim()).unwrap_or(
            DynamicPodcastReframingResult {
                avg_center_x: 0.5,
                face_detected: false,
                width: None,
                height: None,
                podcast: None,
            },
        );

        if res.face_detected || res.podcast.is_some() {
            let _ = cache.put(&cache_key, "podcast_analysis", &res);
        }

        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_tracker_binary() {
        // Binary resolution should succeed when run in the project or installed app
        let res = DynamicPodcastReframing::resolve_tracker_binary();
        assert!(
            res.is_ok(),
            "Tracker binary should resolve: {:?}",
            res.err()
        );
    }
}
