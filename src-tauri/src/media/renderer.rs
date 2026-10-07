use anyhow::{anyhow, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    audio::{build_studio_audio_filter, detect_silences},
    captions::build_ass_filter,
    encoder::{apply_video_encoder_args, supports_videotoolbox},
    face_tracker::{detect_face_center_x, detect_faces_full, FaceTrackerResult},
    ffmpeg::{command_exists, resolve_binary},
    filters::{build_center_crop_filter, build_smart_face_crop_filter},
    presets::OutputPreset,
    probe::probe_media,
    render_plan::{CaptionPlan, ReframePlan, RenderPlan},
};

pub struct TempDirGuard(Option<PathBuf>);

impl TempDirGuard {
    pub fn new(path: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&path);
        Self(Some(path))
    }

    pub fn path(&self) -> &Path {
        self.0.as_ref().expect("temp dir path")
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

pub fn cleanup_stale_temp_dirs() {
    let temp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(temp_dir) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.starts_with("clipon_track_")
                    || name.starts_with("clipon_job_")
                    || name.starts_with("clipon_reframe_")
                    || name.starts_with("clipon_pod_")
                {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
    }
}


pub fn escape_ffmpeg_concat_file_entry(path: &Path) -> String {
    let raw = path.to_string_lossy();
    // In FFmpeg concat demuxer format, special characters such as ' and \
    // inside the single-quoted directive must be escaped with a backslash.
    let escaped = raw.replace('\\', "\\\\").replace('\'', "\\'");
    format!("file '{escaped}'\n")
}

pub fn execute_command_cancellable(
    mut cmd: Command,
    job_id: Option<&str>,
) -> std::io::Result<std::process::Output> {
    if let Some(id) = job_id {
        if crate::jobs::JobManager::is_cancelled_global(id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Job cancelled by user",
            ));
        }
    }
    let child = cmd.spawn()?;
    let pid = child.id();
    if let Some(id) = job_id {
        crate::jobs::JobManager::register_process_global(id, pid);
    }
    let res = child.wait_with_output();
    if let Some(id) = job_id {
        crate::jobs::JobManager::unregister_process_global(id, pid);
    }
    res
}

pub fn execute_render_plan(plan: &RenderPlan) -> Result<PathBuf> {
    if !command_exists("ffmpeg") {
        return Err(anyhow!("ffmpeg is not installed or available on PATH. Please install via Homebrew: 'brew install ffmpeg'"));
    }

    if let Some(parent) = plan.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let first_segment = plan
        .timeline
        .first()
        .ok_or_else(|| anyhow!("RenderPlan timeline cannot be empty"))?;
    let start_sec = first_segment.start_sec;
    let end_sec = first_segment.end_sec;

    let start = format!("{start_sec:.3}");
    let duration_sec = (end_sec - start_sec).max(0.1);
    let duration = format!("{duration_sec:.3}");

    let probe = probe_media(&plan.source)
        .with_context(|| format!("Failed to probe source media '{}'", plan.source))?;
    let has_video = probe.has_video;

    if !has_video && !matches!(plan.reframe, ReframePlan::Original) {
        return Err(anyhow!(
            "Source media '{}' does not contain a video stream required for visual reframing",
            plan.source
        ));
    }

    // Optional dead-air jump-cut filter
    let jump_cuts = if plan.audio.remove_silence {
        let silences = detect_silences(&plan.source, start_sec, duration_sec);
        crate::pro_editor::build_silence_jumpcut_filter(&silences, duration_sec)
    } else {
        None
    };

    let tracker_info = match plan.reframe {
        ReframePlan::SmartFaceTrack => {
            detect_faces_full(&plan.source, start_sec, duration_sec)
        }
        _ => FaceTrackerResult::default(),
    };

    let run_render = |use_videotoolbox: bool| -> Result<()> {
        let mut cmd = Command::new(resolve_binary("ffmpeg"));
        cmd.arg("-nostdin");
        cmd.args(["-y", "-ss", &start, "-i", &plan.source, "-t", &duration]);

        if has_video {
            match &plan.reframe {
                ReframePlan::Original => {
                    let iw_i = probe.width.unwrap_or(1920);
                    let ih_i = probe.height.unwrap_or(1080);
                    let mut filter = "scale='2*trunc(iw/2)':'2*trunc(ih/2)'".to_string();
                    if plan.punch_zoom {
                        filter = format!(
                            "{},{}",
                            filter,
                            crate::pro_editor::build_punch_zoom_filter(iw_i, ih_i)
                        );
                    }
                    if let Some(CaptionPlan::AssSubtitle(ass)) = &plan.captions {
                        if let Some(ass_filter) = build_ass_filter(ass) {
                            filter = format!("{},{}", filter, ass_filter);
                        }
                    } else if let Some(CaptionPlan::Drawtext(drawtext)) = &plan.captions {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    if let Some((ref v_jump, _)) = jump_cuts {
                        filter = format!("{},{}", filter, v_jump);
                    }
                    cmd.args(["-vf", &filter]);
                }
                ReframePlan::SmartFaceTrack => {
                    let iw_f = probe.width.unwrap_or(1920) as f64;
                    let ih_f = probe.height.unwrap_or(1080) as f64;
                    let cw = (iw_f.min(ih_f * 9.0 / 16.0) / 2.0).floor() * 2.0;
                    let ch = (ih_f.min(iw_f * 16.0 / 9.0) / 2.0).floor() * 2.0;

                    let cache = crate::analysis_cache::AnalysisCache::global();
                    let full_dur = probe.duration_sec.unwrap_or(duration_sec);
                    let cache_key = super::active_speaker::compute_active_speaker_cache_key(
                        &plan.source,
                        0.0,
                        full_dur,
                    );
                    let mut active_timeline_opt = cache
                        .get::<super::active_speaker::ActiveSpeakerTimeline>(&cache_key, "active_speaker");

                    if active_timeline_opt.is_none() {
                        let clip_cache_key = super::active_speaker::compute_active_speaker_cache_key(
                            &plan.source,
                            start_sec,
                            duration_sec,
                        );
                        active_timeline_opt = cache.get::<super::active_speaker::ActiveSpeakerTimeline>(&clip_cache_key, "active_speaker");
                    }
                    let active_timeline = active_timeline_opt.unwrap_or_default();

                    let keyframes = super::active_speaker::generate_speaker_aware_keyframes(
                        &active_timeline,
                        &tracker_info,
                        start_sec,
                        start_sec + duration_sec,
                    );

                    let mut filter = if !keyframes.is_empty() {
                        let (expr_x, expr_y) = super::filters::build_dynamic_crop_expr(
                            Some(&keyframes),
                            iw_f,
                            ih_f,
                            cw,
                            ch,
                            tracker_info.avg_center_x,
                            0.38,
                        );
                        format!(
                            "crop=w='2*trunc(min(iw,ih*9/16)/2)':h='2*trunc(min(ih,iw*16/9)/2)':x={}:y={},scale=1080:1920",
                            expr_x, expr_y
                        )
                    } else {
                        let center_x = if tracker_info.face_detected {
                            tracker_info.avg_center_x.clamp(0.20, 0.80)
                        } else {
                            detect_face_center_x(&plan.source, start_sec, duration_sec)
                        };
                        build_smart_face_crop_filter(center_x)
                    };
                    if plan.punch_zoom {
                        filter = format!(
                            "{},{}",
                            filter,
                            crate::pro_editor::build_punch_zoom_filter(1080, 1920)
                        );
                    }
                    if let Some(CaptionPlan::AssSubtitle(ass)) = &plan.captions {
                        if let Some(ass_filter) = build_ass_filter(ass) {
                            filter = format!("{},{}", filter, ass_filter);
                        }
                    } else if let Some(CaptionPlan::Drawtext(drawtext)) = &plan.captions {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    if let Some((ref v_jump, _)) = jump_cuts {
                        filter = format!("{},{}", filter, v_jump);
                    }
                    cmd.args(["-vf", &filter]);
                }
                ReframePlan::VerticalCrop => {
                    let mut filter = build_center_crop_filter();
                    if plan.punch_zoom {
                        filter = format!(
                            "{},{}",
                            filter,
                            crate::pro_editor::build_punch_zoom_filter(1080, 1920)
                        );
                    }
                    if let Some(CaptionPlan::AssSubtitle(ass)) = &plan.captions {
                        if let Some(ass_filter) = build_ass_filter(ass) {
                            filter = format!("{},{}", filter, ass_filter);
                        }
                    } else if let Some(CaptionPlan::Drawtext(drawtext)) = &plan.captions {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    if let Some((ref v_jump, _)) = jump_cuts {
                        filter = format!("{},{}", filter, v_jump);
                    }
                    cmd.args(["-vf", &filter]);
                }
                ReframePlan::SplitScreen => {
                    let iw_f = probe.width.unwrap_or(1920) as f64;
                    let ih_f = probe.height.unwrap_or(1080) as f64;

                    let tracker_info = super::face_tracker::detect_faces_full(
                        &plan.source,
                        start_sec,
                        duration_sec,
                    );
                    let people = tracker_info.people();

                    let assigned_ids: Vec<usize> = if people.len() >= 3 {
                        vec![1, 2, 3]
                    } else if people.len() >= 2 {
                        vec![1, 2]
                    } else {
                        vec![1]
                    };

                    let default_layout = match assigned_ids.len() {
                        3 => "split_three",
                        2 => "split_two",
                        _ => "single",
                    };

                    let top_fb_x = tracker_info
                        .tracking
                        .as_ref()
                        .and_then(|t| t.top_center_x)
                        .unwrap_or(0.26);
                    let top_fb_y = tracker_info
                        .tracking
                        .as_ref()
                        .and_then(|t| t.top_center_y)
                        .unwrap_or(0.38);
                    let bot_fb_x = tracker_info
                        .tracking
                        .as_ref()
                        .and_then(|t| t.bottom_center_x)
                        .unwrap_or(0.78);
                    let bot_fb_y = tracker_info
                        .tracking
                        .as_ref()
                        .and_then(|t| t.bottom_center_y)
                        .unwrap_or(0.38);

                    let base_filter = super::filters::build_multi_speaker_layout_filter_graph(
                        default_layout,
                        &assigned_ids,
                        people,
                        iw_f,
                        ih_f,
                        0.0,
                        duration_sec,
                        top_fb_x,
                        top_fb_y,
                        bot_fb_x,
                        bot_fb_y,
                    );

                    let mut filter_graph = format!("{}[v_split]", base_filter);
                    let mut current_video_stream = "[v_split]".to_string();

                    if plan.punch_zoom {
                        let punch = crate::pro_editor::build_punch_zoom_filter(1080, 1920);
                        filter_graph = format!(
                            "{};{}{}[v_punch]",
                            filter_graph, current_video_stream, punch
                        );
                        current_video_stream = "[v_punch]".to_string();
                    }

                    if let Some(CaptionPlan::AssSubtitle(ass)) = &plan.captions {
                        if let Some(ass_filter) = build_ass_filter(ass) {
                            filter_graph = format!(
                                "{};{}{}[v_sub]",
                                filter_graph, current_video_stream, ass_filter
                            );
                            current_video_stream = "[v_sub]".to_string();
                        }
                    } else if let Some(CaptionPlan::Drawtext(drawtext)) = &plan.captions {
                        if !drawtext.is_empty() {
                            filter_graph = format!(
                                "{};{}{}[v_draw]",
                                filter_graph, current_video_stream, drawtext
                            );
                            current_video_stream = "[v_draw]".to_string();
                        }
                    }

                    if let Some((ref v_jump, _)) = jump_cuts {
                        filter_graph = format!(
                            "{};{}{}[v_jump]",
                            filter_graph, current_video_stream, v_jump
                        );
                        current_video_stream = "[v_jump]".to_string();
                    }

                    cmd.args(["-filter_complex", &filter_graph, "-map", &current_video_stream]);
                }
            }

            apply_video_encoder_args(&mut cmd, use_videotoolbox, plan.output.video_bitrate_kbps);
        } else {
            cmd.arg("-vn");
        }

        let mut audio_filters = Vec::new();
        if let Some((_, ref a_jump)) = jump_cuts {
            audio_filters.push(a_jump.clone());
        }
        if plan.audio.studio_audio {
            audio_filters.push(build_studio_audio_filter());
        }

        if !audio_filters.is_empty() {
            cmd.args([
                "-af",
                &audio_filters.join(","),
                "-c:a",
                "aac",
                "-b:a",
                &format!("{}k", plan.output.audio_bitrate_kbps),
            ]);
        } else {
            cmd.args([
                "-c:a",
                "aac",
                "-b:a",
                &format!("{}k", plan.output.audio_bitrate_kbps),
            ]);
        }
        cmd.arg(&plan.output_path);

        let output = execute_command_cancellable(cmd, plan.job_id.as_deref())
            .context("running ffmpeg clip render")?;
        if !output.status.success() {
            if let Some(id) = plan.job_id.as_deref() {
                if crate::jobs::JobManager::is_cancelled_global(id) {
                    let _ = std::fs::remove_file(&plan.output_path);
                    return Err(anyhow!("Job cancelled by user"));
                }
            }
            return Err(anyhow!(
                "ffmpeg clip render failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    };

    if let Some(id) = plan.job_id.as_deref() {
        if crate::jobs::JobManager::is_cancelled_global(id) {
            let _ = std::fs::remove_file(&plan.output_path);
            return Err(anyhow!("Job cancelled by user"));
        }
    }

    let vt_available = supports_videotoolbox();
    if vt_available {
        if let Err(vt_err) = run_render(true) {
            if let Some(id) = plan.job_id.as_deref() {
                if crate::jobs::JobManager::is_cancelled_global(id) {
                    let _ = std::fs::remove_file(&plan.output_path);
                    return Err(anyhow!("Job cancelled by user"));
                }
            }
            eprintln!(
                "VideoToolbox hardware render failed: {vt_err}. Retrying with software libx264..."
            );
            run_render(false)?;
        }
    } else {
        run_render(false)?;
    }

    if let Some(id) = plan.job_id.as_deref() {
        if crate::jobs::JobManager::is_cancelled_global(id) {
            let _ = std::fs::remove_file(&plan.output_path);
            return Err(anyhow!("Job cancelled by user"));
        }
    }

    Ok(plan.output_path.clone())
}

pub fn render_flat_clip_with_job(
    source_path: &str,
    start_sec: f64,
    end_sec: f64,
    output_path: &Path,
    drawtext_filters: Option<&str>,
    ass_subtitle_path: Option<&Path>,
    reframe_mode: Option<&str>,
    remove_silence: bool,
    punch_zoom: bool,
    studio_audio: bool,
    job_id: Option<String>,
    export_preset: Option<&str>,
) -> Result<PathBuf> {
    let mode = reframe_mode.unwrap_or("vertical_crop");
    let effective_punch = punch_zoom || mode == "punch_zoom";

    let captions = if let Some(ass) = ass_subtitle_path {
        Some(CaptionPlan::AssSubtitle(ass.to_path_buf()))
    } else if let Some(drawtext) = drawtext_filters {
        Some(CaptionPlan::Drawtext(drawtext.to_string()))
    } else {
        None
    };

    let preset = export_preset
        .map(OutputPreset::from_name)
        .unwrap_or_else(OutputPreset::instagram_reels);

    let mut plan = RenderPlan::single_clip(
        source_path,
        start_sec,
        end_sec,
        output_path.to_path_buf(),
        ReframePlan::from_mode_str(reframe_mode),
        captions,
        super::render_plan::AudioPlan {
            studio_audio,
            remove_silence,
        },
        effective_punch,
        preset,
    );
    plan.job_id = job_id;

    execute_render_plan(&plan)
}

pub fn render_flat_clip(
    source_path: &str,
    start_sec: f64,
    end_sec: f64,
    output_path: &Path,
    drawtext_filters: Option<&str>,
    ass_subtitle_path: Option<&Path>,
    reframe_mode: Option<&str>,
    remove_silence: bool,
    punch_zoom: bool,
    studio_audio: bool,
) -> Result<PathBuf> {
    render_flat_clip_with_job(
        source_path,
        start_sec,
        end_sec,
        output_path,
        drawtext_filters,
        ass_subtitle_path,
        reframe_mode,
        remove_silence,
        punch_zoom,
        studio_audio,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_dir_guard_lifecycle() {
        let path = std::env::temp_dir().join(format!("clipon_reframe_test_{}", uuid::Uuid::new_v4()));
        {
            let guard = TempDirGuard::new(path.clone());
            assert!(guard.path().exists());
            std::fs::write(guard.path().join("dummy.txt"), "test").unwrap();
        }
        assert!(!path.exists());
    }

    #[test]
    fn test_cleanup_stale_temp_dirs() {
        let p1 = std::env::temp_dir().join(format!("clipon_track_stale_{}", uuid::Uuid::new_v4()));
        let p2 = std::env::temp_dir().join(format!("clipon_reframe_stale_{}", uuid::Uuid::new_v4()));
        let p3 = std::env::temp_dir().join(format!("clipon_job_stale_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p1).unwrap();
        std::fs::create_dir_all(&p2).unwrap();
        std::fs::create_dir_all(&p3).unwrap();
        assert!(p1.exists() && p2.exists() && p3.exists());

        cleanup_stale_temp_dirs();
        assert!(!p1.exists() && !p2.exists() && !p3.exists());
    }

    #[test]
    fn test_escape_ffmpeg_concat_file_entry() {
        let p1 = Path::new("/path/to/my video's file.mp4");
        let entry1 = escape_ffmpeg_concat_file_entry(p1);
        assert_eq!(entry1, "file '/path/to/my video\\'s file.mp4'\n");

        let p2 = Path::new("C:\\Users\\User\\Videos\\clip's.mp4");
        let entry2 = escape_ffmpeg_concat_file_entry(p2);
        assert_eq!(entry2, "file 'C:\\\\Users\\\\User\\\\Videos\\\\clip\\'s.mp4'\n");
    }
}
