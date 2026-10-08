pub mod active_speaker;
pub mod audio;
pub mod captions;
pub mod encoder;
pub mod face_tracker;
pub mod ffmpeg;
pub mod filters;
pub mod presets;
pub mod probe;
pub mod render_plan;
pub mod renderer;

// Re-exports for public API backwards compatibility
#[allow(unused_imports)]
pub use active_speaker::{
    compute_active_speaker_cache_key, generate_speaker_aware_keyframes,
    get_or_compute_active_speaker_timeline, validate_clip_visuals, ActiveSpeakerDetector,
    ActiveSpeakerProvider, ActiveSpeakerSegment, ActiveSpeakerService, ActiveSpeakerTimeline,
    LocalFallbackProvider, LocalFusionDetector, NormalizedAsdResult, NvidiaAsdDetector,
    NvidiaAsdProvider, NvidiaAudioStreamMode, VisualValidationResult,
};
#[allow(unused_imports)]
pub use audio::{detect_silences, extract_audio};
#[allow(unused_imports)]
pub use encoder::{detect_hardware_capabilities, supports_videotoolbox, HardwareCapabilities};
#[allow(unused_imports)]
pub use face_tracker::{
    detect_face_center_x, detect_faces_full, FaceTracker, FaceTrackerResult,
};
#[allow(unused_imports)]
pub use ffmpeg::{command_exists, resolve_binary};
#[allow(unused_imports)]
pub use filters::{
    build_center_crop_filter, build_dynamic_crop_expr, build_multi_speaker_layout_filter_graph,
    build_original_scale_filter, build_smart_face_crop_filter, PersonKeyframe, TrackingKeyframe,
};
#[allow(unused_imports)]
pub use presets::{ExportPlatform, OutputPreset};
#[allow(unused_imports)]
pub use probe::{probe_media, validate_rendered_output};
#[allow(unused_imports)]
pub use render_plan::{AudioPlan, CaptionPlan, ReframePlan, RenderPlan, TimelineSegment};
#[allow(unused_imports)]
pub use renderer::{
    cleanup_stale_temp_dirs, execute_render_plan, render_flat_clip, render_flat_clip_with_job,
    TempDirGuard,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dynamic_crop_expr_fallback() {
        let (expr_x, expr_y) =
            build_dynamic_crop_expr(None, 1920.0, 1080.0, 608.0, 540.0, 0.26, 0.38);
        assert!(expr_x.starts_with('\'') && expr_x.ends_with('\''));
        assert!(expr_y.starts_with('\'') && expr_y.ends_with('\''));
        assert_eq!(expr_x, "'194'");
    }

    #[test]
    fn test_build_dynamic_crop_expr_keyframes() {
        let kfs = vec![
            TrackingKeyframe {
                t: 0.0,
                x: 0.50,
                y: 0.38,
            },
            TrackingKeyframe {
                t: 5.0,
                x: 0.26,
                y: 0.38,
            },
            TrackingKeyframe {
                t: 10.0,
                x: 0.26,
                y: 0.38,
            },
        ];
        let (expr_x, expr_y) =
            build_dynamic_crop_expr(Some(&kfs), 1920.0, 1080.0, 608.0, 540.0, 0.26, 0.38);
        assert!(expr_x.contains("if(lt(t,"));
        assert!(expr_x.contains("2*trunc("));
        assert!(expr_y.contains("2*trunc("));
    }

    #[test]
    fn test_render_plan_construction() {
        let plan = RenderPlan::single_clip(
            "/path/to/test.mp4",
            10.0,
            25.0,
            std::path::PathBuf::from("/path/to/out.mp4"),
            ReframePlan::SmartFaceTrack,
            None,
            AudioPlan {
                studio_audio: true,
                remove_silence: false,
            },
            true,
            OutputPreset::instagram_reels(),
        );
        assert_eq!(plan.total_duration(), 15.0);
        assert_eq!(plan.output.width, 1080);
        assert_eq!(plan.output.height, 1920);
        assert!(plan.punch_zoom);
    }
}
