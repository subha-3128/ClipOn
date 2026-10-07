//! Issue #18: Golden test cases for all reframe modes
//!
//! Renders short test clips (1.5-2 seconds) across all 3 reframe modes:
//! - Original
//! - VerticalCrop
//! - SmartFaceTrack
//!
//! Validates:
//! - Output file exists and is non-empty
//! - Video stream aspect ratio matches expected geometry (1080x1920 for 9:16 vertical modes)
//! - Audio stream is preserved

use std::path::PathBuf;
use clipon_lib::media::{probe_media, render_flat_clip};

fn repo_root() -> PathBuf {
    let mut dir = std::env::current_dir().expect("current dir");
    for _ in 0..4 {
        if dir.join("src-tauri").is_dir() && dir.join("package.json").is_file() {
            return dir;
        }
        dir = dir.parent().expect("parent dir").to_path_buf();
    }
    dir
}

fn test_video() -> Option<PathBuf> {
    let root = repo_root();
    let p2 = root.join("testvideo2.mp4");
    if p2.exists() {
        return Some(p2);
    }
    let p1 = root.join("testvideo.mp4");
    if p1.exists() {
        return Some(p1);
    }

    // Attempt to generate synthetic test fixture if ffmpeg is available
    let synthetic_path = out_dir().join("synthetic_golden_fixture.mp4");
    if synthetic_path.exists()
        && std::fs::metadata(&synthetic_path).map(|m| m.len() > 1000).unwrap_or(false)
    {
        return Some(synthetic_path);
    }

    if clipon_lib::media::command_exists("ffmpeg") {
        let ffmpeg = clipon_lib::media::resolve_binary("ffmpeg");
        let gen_status = std::process::Command::new(ffmpeg)
            .args(&[
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=5:size=1920x1080:rate=30",
                "-f", "lavfi",
                "-i", "sine=frequency=1000:duration=5",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                "-c:a", "aac",
                "-shortest",
                synthetic_path.to_str().unwrap(),
            ])
            .output();

        if let Ok(out) = gen_status {
            if out.status.success() && synthetic_path.exists() {
                return Some(synthetic_path);
            }
        }
    }

    // In CI or when fixtures are strictly required, fail the test instead of silently passing!
    if std::env::var("CI").is_ok() || std::env::var("REQUIRE_FIXTURES").is_ok() {
        panic!("CI FAILURE: Video fixture absent and could not be generated. Integration tests must not silently pass in CI!");
    }

    None
}

fn out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("clipon_reframe_golden_tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_reframe_golden_original() {
    let Some(source) = test_video() else {
        eprintln!("SKIPPED test_reframe_golden_original: test video not present");
        return;
    };

    let out = out_dir().join("golden_original.mp4");
    let _ = std::fs::remove_file(&out);

    let res = render_flat_clip(
        source.to_str().unwrap(),
        0.0,
        2.0,
        &out,
        None,
        None,
        Some("original"),
        false,
        false,
        false,
    );
    assert!(res.is_ok(), "render failed: {:?}", res.err());
    assert!(out.exists(), "output file must exist");
    assert!(std::fs::metadata(&out).unwrap().len() > 1000, "output file must not be empty");

    let probe = probe_media(out.to_str().unwrap()).expect("probe output");
    assert!(probe.width.is_some() && probe.height.is_some(), "must have video stream");
    assert!(probe.audio_codec.is_some(), "audio must be preserved");
}

#[test]
fn test_reframe_golden_vertical_crop() {
    let Some(source) = test_video() else {
        eprintln!("SKIPPED test_reframe_golden_vertical_crop: test video not present");
        return;
    };

    let out = out_dir().join("golden_vertical_crop.mp4");
    let _ = std::fs::remove_file(&out);

    let res = render_flat_clip(
        source.to_str().unwrap(),
        0.0,
        2.0,
        &out,
        None,
        None,
        Some("vertical_crop"),
        false,
        false,
        false,
    );
    assert!(res.is_ok(), "render failed: {:?}", res.err());
    assert!(out.exists(), "output file must exist");
    assert!(std::fs::metadata(&out).unwrap().len() > 1000, "output file must not be empty");

    let probe = probe_media(out.to_str().unwrap()).expect("probe output");
    assert_eq!(probe.width, Some(1080), "vertical crop width must be 1080");
    assert_eq!(probe.height, Some(1920), "vertical crop height must be 1920");
    assert!(probe.audio_codec.is_some(), "audio must be preserved in VerticalCrop mode");
}

#[test]
fn test_reframe_golden_smart_face_track() {
    let Some(source) = test_video() else {
        eprintln!("SKIPPED test_reframe_golden_smart_face_track: test video not present");
        return;
    };

    let out = out_dir().join("golden_smart_face_track.mp4");
    let _ = std::fs::remove_file(&out);

    let res = render_flat_clip(
        source.to_str().unwrap(),
        0.0,
        2.0,
        &out,
        None,
        None,
        Some("smart_face_track"),
        false,
        false,
        false,
    );
    assert!(res.is_ok(), "render failed: {:?}", res.err());
    assert!(out.exists(), "output file must exist");
    assert!(std::fs::metadata(&out).unwrap().len() > 1000, "output file must not be empty");

    let probe = probe_media(out.to_str().unwrap()).expect("probe output");
    assert_eq!(probe.width, Some(1080), "smart face track width must be 1080");
    assert_eq!(probe.height, Some(1920), "smart face track height must be 1920");
    assert!(probe.audio_codec.is_some(), "audio must be preserved in SmartFaceTrack mode");
}
