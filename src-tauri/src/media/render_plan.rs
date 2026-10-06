use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::presets::OutputPreset;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSegment {
    pub start_sec: f64,
    pub end_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReframePlan {
    Original,
    VerticalCrop,
    SmartFaceTrack,
    PodcastSplit { layout_override: Option<String> },
}

impl Default for ReframePlan {
    fn default() -> Self {
        Self::VerticalCrop
    }
}

impl ReframePlan {
    pub fn from_mode_str(mode: Option<&str>) -> Self {
        match mode.unwrap_or("vertical_crop") {
            "original" => Self::Original,
            "smart_face_track" => Self::SmartFaceTrack,
            "podcast_split" => Self::PodcastSplit {
                layout_override: None,
            },
            _ => Self::VerticalCrop,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptionPlan {
    AssSubtitle(PathBuf),
    Drawtext(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioPlan {
    pub studio_audio: bool,
    pub remove_silence: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPlan {
    pub source: String,
    pub timeline: Vec<TimelineSegment>,
    pub reframe: ReframePlan,
    pub captions: Option<CaptionPlan>,
    pub audio: AudioPlan,
    pub punch_zoom: bool,
    pub output_path: PathBuf,
    pub output: OutputPreset,
    pub job_id: Option<String>,
}

#[allow(dead_code)]
impl RenderPlan {
    pub fn single_clip(
        source: &str,
        start_sec: f64,
        end_sec: f64,
        output_path: PathBuf,
        reframe: ReframePlan,
        captions: Option<CaptionPlan>,
        audio: AudioPlan,
        punch_zoom: bool,
        output: OutputPreset,
    ) -> Self {
        Self {
            source: source.to_string(),
            timeline: vec![TimelineSegment { start_sec, end_sec }],
            reframe,
            captions,
            audio,
            punch_zoom,
            output_path,
            output,
            job_id: None,
        }
    }

    pub fn total_duration(&self) -> f64 {
        self.timeline
            .iter()
            .map(|s| (s.end_sec - s.start_sec).max(0.0))
            .sum()
    }
}
