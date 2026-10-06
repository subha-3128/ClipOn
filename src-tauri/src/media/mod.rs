pub mod audio;
pub mod captions;
pub mod encoder;
pub mod ffmpeg;
pub mod filters;
pub mod podcast;
pub mod presets;
pub mod probe;
pub mod render_plan;
pub mod renderer;

// Re-exports for public API backwards compatibility
#[allow(unused_imports)]
pub use audio::{detect_silences, extract_audio};
#[allow(unused_imports)]
pub use encoder::{detect_hardware_capabilities, supports_videotoolbox, HardwareCapabilities};
#[allow(unused_imports)]
pub use ffmpeg::{command_exists, resolve_binary};
#[allow(unused_imports)]
pub use filters::{build_dynamic_crop_expr, PodcastKeyframe};
#[allow(unused_imports)]
pub use podcast::{
    build_segment_filter_graph, detect_face_center_x, detect_faces_full, DynamicPodcastReframing,
    DynamicPodcastReframingResult, FaceTrackerResult, LayoutSegment, PersonKeyframe, PersonTrack,
    PodcastFaceTracking,
};
#[allow(unused_imports)]
pub use presets::{ExportPlatform, OutputPreset};
#[allow(unused_imports)]
pub use probe::probe_media;
#[allow(unused_imports)]
pub use render_plan::{AudioPlan, CaptionPlan, ReframePlan, RenderPlan, TimelineSegment};
#[allow(unused_imports)]
pub use renderer::{
    cleanup_stale_temp_dirs, execute_render_plan, merge_adjacent_segments, render_flat_clip,
    render_flat_clip_with_job, TempDirGuard,
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
            PodcastKeyframe {
                t: 0.0,
                x: 0.50,
                y: 0.38,
            },
            PodcastKeyframe {
                t: 5.0,
                x: 0.26,
                y: 0.38,
            },
            PodcastKeyframe {
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
    fn test_build_segment_filter_graph_single() {
        let people = vec![PersonTrack {
            id: 1,
            name: Some("Host".to_string()),
            keyframes: vec![PersonKeyframe::new(0.0, 0.5, 0.38)],
        }];
        let filter = build_segment_filter_graph(
            "single",
            &[1],
            &people,
            1920.0,
            1080.0,
            0.0,
            10.0,
            0.5,
            0.38,
            0.5,
            0.38,
        );
        assert!(filter.contains("crop="));
        assert!(filter.contains("scale=1080:1920"));
    }

    #[test]
    fn test_build_segment_filter_graph_split_two() {
        let people = vec![
            PersonTrack {
                id: 1,
                name: Some("P1".to_string()),
                keyframes: vec![PersonKeyframe::new(0.0, 0.26, 0.38)],
            },
            PersonTrack {
                id: 2,
                name: Some("P2".to_string()),
                keyframes: vec![PersonKeyframe::new(0.0, 0.78, 0.38)],
            },
        ];
        let filter = build_segment_filter_graph(
            "split_two",
            &[1, 2],
            &people,
            1920.0,
            1080.0,
            0.0,
            10.0,
            0.26,
            0.38,
            0.78,
            0.38,
        );
        assert!(filter.contains("[top][bot]vstack"));
        assert!(filter.contains("scale=1080:960"));
        assert!(filter.contains("drawbox="));
    }

    #[test]
    fn test_build_segment_filter_graph_split_three() {
        let people = vec![
            PersonTrack {
                id: 1,
                name: Some("P1".to_string()),
                keyframes: vec![PersonKeyframe::new(0.0, 0.25, 0.38)],
            },
            PersonTrack {
                id: 2,
                name: Some("P2".to_string()),
                keyframes: vec![PersonKeyframe::new(0.0, 0.75, 0.38)],
            },
            PersonTrack {
                id: 3,
                name: Some("P3".to_string()),
                keyframes: vec![PersonKeyframe::new(0.0, 0.50, 0.60)],
            },
        ];
        let filter = build_segment_filter_graph(
            "split_three",
            &[1, 2, 3],
            &people,
            1920.0,
            1080.0,
            0.0,
            10.0,
            0.26,
            0.38,
            0.78,
            0.38,
        );
        assert!(filter.contains("scale=540:960"));
        assert!(filter.contains("[p1][p2]hstack"));
        assert!(filter.contains("[top_row][bot]vstack"));
        assert!(filter.contains("drawbox="));
    }

    #[test]
    fn test_render_plan_construction() {
        let plan = RenderPlan::single_clip(
            "/path/to/test.mp4",
            10.0,
            25.0,
            std::path::PathBuf::from("/path/to/out.mp4"),
            ReframePlan::PodcastSplit {
                layout_override: None,
            },
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
