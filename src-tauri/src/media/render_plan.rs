use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::presets::OutputPreset;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSegment {
    pub start_sec: f64,
    pub end_sec: f64,
}

impl TimelineSegment {
    pub fn is_valid(&self) -> bool {
        self.start_sec >= 0.0 && self.end_sec > self.start_sec
    }

    pub fn duration(&self) -> f64 {
        (self.end_sec - self.start_sec).max(0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReframePlan {
    Original,
    VerticalCrop,
    SmartFaceTrack,
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
            _ => Self::VerticalCrop,
        }
    }

    pub fn validate(&self, _source_aspect_ratio: f64, has_face_data: bool) -> Result<(), &'static str> {
        match self {
            Self::Original | Self::VerticalCrop => Ok(()),
            Self::SmartFaceTrack => {
                if !has_face_data {
                    Err("SmartFaceTrack requires face tracking metadata")
                } else {
                    Ok(())
                }
            }
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

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.timeline.is_empty() {
            return Err("Timeline cannot be empty");
        }
        for (i, seg) in self.timeline.iter().enumerate() {
            if !seg.is_valid() {
                return Err("Timeline segment duration must be positive and non-negative");
            }
            if i > 0 && seg.start_sec < self.timeline[i - 1].end_sec {
                return Err("Timeline segments cannot overlap");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeline_segment_duration_and_validity() {
        let seg = TimelineSegment { start_sec: 1.0, end_sec: 5.5 };
        assert!(seg.is_valid());
        assert_eq!(seg.duration(), 4.5);

        let invalid_neg = TimelineSegment { start_sec: -1.0, end_sec: 5.0 };
        assert!(!invalid_neg.is_valid());

        let invalid_inverted = TimelineSegment { start_sec: 5.0, end_sec: 2.0 };
        assert!(!invalid_inverted.is_valid());
        assert_eq!(invalid_inverted.duration(), 0.0);
    }

    #[test]
    fn test_total_duration_calculation() {
        let mut plan = RenderPlan::single_clip(
            "test.mp4",
            0.0,
            10.0,
            PathBuf::from("out.mp4"),
            ReframePlan::Original,
            None,
            AudioPlan::default(),
            false,
            OutputPreset::youtube_shorts(),
        );
        assert_eq!(plan.total_duration(), 10.0);

        plan.timeline = vec![
            TimelineSegment { start_sec: 0.0, end_sec: 4.0 },
            TimelineSegment { start_sec: 10.0, end_sec: 15.0 },
        ];
        assert_eq!(plan.total_duration(), 9.0);

        plan.timeline = vec![];
        assert_eq!(plan.total_duration(), 0.0);
    }

    #[test]
    fn test_render_plan_validation() {
        let mut plan = RenderPlan::single_clip(
            "test.mp4",
            0.0,
            10.0,
            PathBuf::from("out.mp4"),
            ReframePlan::Original,
            None,
            AudioPlan::default(),
            false,
            OutputPreset::youtube_shorts(),
        );
        assert!(plan.validate().is_ok());

        plan.timeline.clear();
        assert_eq!(plan.validate(), Err("Timeline cannot be empty"));

        plan.timeline = vec![TimelineSegment { start_sec: 5.0, end_sec: 3.0 }];
        assert_eq!(plan.validate(), Err("Timeline segment duration must be positive and non-negative"));

        plan.timeline = vec![
            TimelineSegment { start_sec: 0.0, end_sec: 10.0 },
            TimelineSegment { start_sec: 8.0, end_sec: 15.0 },
        ];
        assert_eq!(plan.validate(), Err("Timeline segments cannot overlap"));
    }

    #[test]
    fn test_reframe_plan_from_mode_str() {
        assert_eq!(ReframePlan::from_mode_str(Some("original")), ReframePlan::Original);
        assert_eq!(ReframePlan::from_mode_str(Some("smart_face_track")), ReframePlan::SmartFaceTrack);
        assert_eq!(ReframePlan::from_mode_str(Some("vertical_crop")), ReframePlan::VerticalCrop);
        assert_eq!(ReframePlan::from_mode_str(Some("unknown_gibberish")), ReframePlan::VerticalCrop);
        assert_eq!(ReframePlan::from_mode_str(None), ReframePlan::VerticalCrop);
    }

    #[test]
    fn test_reframe_plan_validate() {
        assert!(ReframePlan::Original.validate(1.77, false).is_ok());
        assert!(ReframePlan::VerticalCrop.validate(1.77, false).is_ok());

        assert!(ReframePlan::SmartFaceTrack.validate(1.77, true).is_ok());
        assert!(ReframePlan::SmartFaceTrack.validate(1.77, false).is_err());
    }

    #[test]
    fn test_render_plan_serde_roundtrip() {
        let plan = RenderPlan::single_clip(
            "source.mp4",
            5.0,
            15.0,
            PathBuf::from("/out/test.mp4"),
            ReframePlan::SmartFaceTrack,
            None,
            AudioPlan { studio_audio: true, remove_silence: true },
            true,
            OutputPreset::youtube_shorts(),
        );

        let json = serde_json::to_string(&plan).expect("serialize");
        let deserialized: RenderPlan = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.source, "source.mp4");
        assert_eq!(deserialized.timeline.len(), 1);
        assert_eq!(deserialized.reframe, ReframePlan::SmartFaceTrack);
        assert_eq!(deserialized.audio.studio_audio, true);
        assert_eq!(deserialized.punch_zoom, true);
    }
}
