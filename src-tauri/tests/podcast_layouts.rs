//! Issue #5 — Dynamic layout rule validation against real footage.
//!
//! Verifies with testvideo2.mp4 (when present next to the repository root):
//! - 1/2/3-person renders are 1080x1920 9:16 with audio and correct duration
//! - layout selection is driven by continuous tracking, not the first frame
//! - segments are contiguous, cover the analyzed window, and respect min duration
//! - captioned renders keep the same output geometry and timing
//!
//! The test self-skips when the footage is unavailable so CI on other machines
//! still passes; on this repository's machine it must run against the video.

use std::path::{Path, PathBuf};

use clipon_lib::media::{probe_media, render_flat_clip, DynamicPodcastReframing};
use clipon_lib::models::TranscriptWord;
use clipon_lib::pro_editor::generate_kinetic_ass;

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

fn testvideo2() -> Option<PathBuf> {
    let p = repo_root().join("testvideo2.mp4");
    p.exists().then_some(p)
}

fn out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("clipon_issue5_layout_tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Window {
    name: &'static str,
    start: f64,
    end: f64,
    expected_layout: &'static str,
    expected_ids: &'static [usize],
}

const WINDOWS: &[Window] = &[
    Window { name: "single", start: 576.0, end: 586.0, expected_layout: "single", expected_ids: &[1] },
    Window { name: "split_two", start: 196.0, end: 206.0, expected_layout: "split_two", expected_ids: &[1, 2] },
    Window { name: "split_three", start: 220.0, end: 230.0, expected_layout: "split_three", expected_ids: &[1, 2, 3] },
];

fn assert_segments_valid(segments: &[clipon_lib::dynamic_podcast_reframing::LayoutSegment], duration_sec: f64) {
    assert!(!segments.is_empty(), "segments must not be empty");
    assert!(segments[0].start <= 0.05, "segments must start at 0 (got {})", segments[0].start);
    assert!(
        (segments.last().unwrap().end - duration_sec).abs() <= 0.1,
        "segments must cover the analyzed window (last end {} vs duration {duration_sec})",
        segments.last().unwrap().end
    );
    for pair in segments.windows(2) {
        assert!(
            (pair[0].end - pair[1].start).abs() <= 0.02,
            "segments must be contiguous: {} -> {}",
            pair[0].end,
            pair[1].start
        );
    }
    for seg in segments {
        assert_eq!(seg.layout_type, layout_type_for(seg.person_ids.len()), "layout_type must match person count");
        let dur = seg.end - seg.start;
        assert!(dur >= 2.99, "segment {seg:?} shorter than 3s minimum");
    }
}

fn layout_type_for(n: usize) -> &'static str {
    match n {
        1 => "single",
        2 => "split_two",
        _ => "split_three",
    }
}

fn synthetic_words(start: f64, end: f64) -> Vec<TranscriptWord> {
    let mut words = Vec::new();
    let mut t = start;
    let texts = ["CLIPON", "LAYOUT", "VALIDATION", "CAPTIONS", "SYNCED"];
    let mut i = 0;
    while t + 0.8 < end {
        words.push(TranscriptWord {
            text: texts[i % texts.len()].to_string(),
            start: t,
            end: t + 0.75,
            speaker: None,
        });
        t += 0.8;
        i += 1;
    }
    words
}

#[test]
fn podcast_layout_rules_against_real_footage() {
    let Some(video) = testvideo2() else {
        eprintln!("SKIPPED: testvideo2.mp4 not present");
        return;
    };

    for w in WINDOWS {
        let duration = w.end - w.start;
        let result = DynamicPodcastReframing::analyze(
            video.to_str().unwrap(),
            w.start,
            duration,
        );
        let podcast = result.podcast.as_ref().expect("podcast analysis result");
        let segments = podcast.segments.as_ref().expect("segments");
        assert_segments_valid(segments, duration);

        // The expected layout must be the final, dominant state of the window.
        // A short (<= min layout duration) `single` intro is legitimate: identity
        // confirmation plus the 3s minimum keep the initial segment stable while
        // tracks are being confirmed within this window.
        let last = segments.last().unwrap();
        assert_eq!(last.layout_type, w.expected_layout, "{} final layout", w.name);
        assert_eq!(last.person_ids, w.expected_ids, "{} final ids", w.name);
        let covered: f64 = segments
            .iter()
            .filter(|s| s.layout_type == w.expected_layout && s.person_ids == w.expected_ids)
            .map(|s| s.end - s.start)
            .sum();
        assert!(
            covered >= duration * 0.5,
            "{}: expected layout covers only {covered:.1}s of {duration:.1}s",
            w.name
        );
        if w.name == "single" {
            assert_eq!(segments.len(), 1, "single window must stay single throughout");
        }

        // Plain render
        let out = out_dir().join(format!("issue5_{}.mp4", w.name));
        render_flat_clip(
            video.to_str().unwrap(),
            w.start,
            w.end,
            &out,
            None,
            None,
            Some("podcast_split"),
            false,
            false,
            true,
        )
        .expect("render must succeed");
        assert_rendered_vertical_clip(&out, duration, w.name);

        // Captioned render: same geometry, same timing, captions burned in
        let ass = out_dir().join(format!("issue5_{}.ass", w.name));
        std::fs::write(&ass, generate_kinetic_ass(&synthetic_words(w.start, w.end), w.start, w.end, "hormozi-kinetic", w.name != "single"))
            .unwrap();
        let out_cap = out_dir().join(format!("issue5_{}_captions.mp4", w.name));
        render_flat_clip(
            video.to_str().unwrap(),
            w.start,
            w.end,
            &out_cap,
            None,
            Some(&ass),
            Some("podcast_split"),
            false,
            false,
            true,
        )
        .expect("captioned render must succeed");
        assert_rendered_vertical_clip(&out_cap, duration, w.name);
    }
}

#[test]
fn podcast_layout_tracks_continuously_across_transitions() {
    let Some(video) = testvideo2() else {
        eprintln!("SKIPPED: testvideo2.mp4 not present");
        return;
    };
    // 44.0–70.0 spans single -> split_three -> split_two in this footage.
    let start = 44.0;
    let duration = 26.0;
    let result = DynamicPodcastReframing::analyze(video.to_str().unwrap(), start, duration);
    let podcast = result.podcast.as_ref().expect("podcast analysis result");
    let segments = podcast.segments.as_ref().expect("segments");
    assert_segments_valid(segments, duration);

    let layouts: std::collections::BTreeSet<String> =
        segments.iter().map(|s| s.layout_type.clone()).collect();
    assert!(
        layouts.len() > 1,
        "layout must follow continuous tracking across the window, got static {layouts:?}"
    );

    let out = out_dir().join("issue5_transition.mp4");
    render_flat_clip(
        video.to_str().unwrap(),
        start,
        start + duration,
        &out,
        None,
        None,
        Some("podcast_split"),
        false,
        false,
        true,
    )
    .expect("transition render must succeed");
    assert_rendered_vertical_clip(&out, duration, "transition");
}

fn assert_rendered_vertical_clip(path: &Path, expected_duration: f64, label: &str) {
    let probe = probe_media(path.to_str().unwrap()).expect("probing rendered clip");
    assert!(probe.has_video, "{label}: rendered clip has no video");
    assert!(probe.audio_codec.is_some(), "{label}: rendered clip has no audio");
    assert_eq!(probe.width, Some(1080), "{label}: width must be 1080");
    assert_eq!(probe.height, Some(1920), "{label}: height must be 1920");
    let dur = probe.duration_sec.expect("duration");
    assert!(
        (dur - expected_duration).abs() <= 1.5,
        "{label}: duration {dur} differs from expected {expected_duration}"
    );
}
