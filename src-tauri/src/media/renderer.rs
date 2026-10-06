use anyhow::{anyhow, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use super::{
    audio::{build_studio_audio_filter, detect_silences},
    captions::build_ass_filter,
    encoder::{apply_video_encoder_args, supports_videotoolbox},
    ffmpeg::{command_exists, resolve_binary},
    filters::{build_center_crop_filter, build_smart_face_crop_filter},
    podcast::{
        build_segment_filter_graph, detect_face_center_x, detect_faces_full, FaceTrackerResult,
    },
    presets::OutputPreset,
    probe::probe_media,
    render_plan::{CaptionPlan, ReframePlan, RenderPlan},
};

pub fn execute_render_plan(plan: &RenderPlan) -> Result<PathBuf> {
    if !command_exists("ffmpeg") {
        return Err(anyhow!("ffmpeg is not installed or not available on PATH"));
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

    let probe = probe_media(&plan.source).ok();
    let has_video = probe.as_ref().map(|p| p.has_video).unwrap_or(false);

    // Optional dead-air jump-cut filter
    let jump_cuts = if plan.audio.remove_silence {
        let silences = detect_silences(&plan.source, start_sec, duration_sec);
        crate::pro_editor::build_silence_jumpcut_filter(&silences, duration_sec)
    } else {
        None
    };

    let tracker_info = match plan.reframe {
        ReframePlan::PodcastSplit { .. } | ReframePlan::SmartFaceTrack => {
            detect_faces_full(&plan.source, start_sec, duration_sec)
        }
        _ => FaceTrackerResult {
            avg_center_x: 0.5,
            face_detected: false,
            width: None,
            height: None,
            podcast: None,
        },
    };

    let run_render = |use_videotoolbox: bool| -> Result<()> {
        let mut cmd = Command::new(resolve_binary("ffmpeg"));
        cmd.arg("-nostdin");
        cmd.args(["-y", "-ss", &start, "-i", &plan.source, "-t", &duration]);

        if has_video {
            match &plan.reframe {
                ReframePlan::Original => {
                    let iw_i = probe.as_ref().and_then(|p| p.width).unwrap_or(1920) as i64;
                    let ih_i = probe.as_ref().and_then(|p| p.height).unwrap_or(1080) as i64;
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
                ReframePlan::PodcastSplit { .. } => {
                    let iw_f = probe.as_ref().and_then(|p| p.width).unwrap_or(1920) as f64;
                    let ih_f = probe.as_ref().and_then(|p| p.height).unwrap_or(1080) as f64;

                    let pod = tracker_info.podcast.as_ref();
                    let people = pod.and_then(|p| p.people.as_deref()).unwrap_or(&[]);
                    let segments = pod.and_then(|p| p.segments.as_deref()).unwrap_or(&[]);

                    let top_fallback_x = pod.map(|p| p.top_center_x).unwrap_or(0.26);
                    let top_fallback_y = pod.map(|p| p.top_center_y).unwrap_or(0.38);
                    let bot_fallback_x = pod.map(|p| p.bottom_center_x).unwrap_or(0.78);
                    let bot_fallback_y = pod.map(|p| p.bottom_center_y).unwrap_or(0.38);

                    let has_multi_layout = segments.len() > 1 && {
                        let first_type = &segments[0].layout_type;
                        let first_ids = &segments[0].person_ids;
                        segments
                            .iter()
                            .any(|s| &s.layout_type != first_type || &s.person_ids != first_ids)
                    };

                    if has_multi_layout {
                        let temp_dir = std::env::temp_dir()
                            .join(format!("clipon_pod_{}", uuid::Uuid::new_v4()));
                        let _ = std::fs::create_dir_all(&temp_dir);

                        let mut seg_files = Vec::new();
                        for (idx, seg) in segments.iter().enumerate() {
                            let seg_dur = (seg.end - seg.start).max(0.1);
                            let seg_abs_start = start_sec + seg.start;
                            let seg_filter = format!(
                                "{}[seg_out]",
                                build_segment_filter_graph(
                                    &seg.layout_type,
                                    &seg.person_ids,
                                    people,
                                    iw_f,
                                    ih_f,
                                    seg.start,
                                    seg.end,
                                    top_fallback_x,
                                    top_fallback_y,
                                    bot_fallback_x,
                                    bot_fallback_y,
                                )
                            );
                            let seg_out_path = temp_dir.join(format!("seg_{:03}.mp4", idx));
                            let mut seg_cmd = Command::new(resolve_binary("ffmpeg"));
                            seg_cmd.arg("-nostdin");
                            seg_cmd.args([
                                "-y",
                                "-ss",
                                &format!("{seg_abs_start:.3}"),
                                "-t",
                                &format!("{seg_dur:.3}"),
                                "-i",
                                &plan.source,
                                "-filter_complex",
                                &seg_filter,
                                "-map",
                                "[seg_out]",
                                "-an",
                            ]);
                            apply_video_encoder_args(
                                &mut seg_cmd,
                                use_videotoolbox,
                                plan.output.video_bitrate_kbps,
                            );
                            seg_cmd.arg(&seg_out_path);
                            if seg_cmd
                                .output()
                                .map(|o| o.status.success())
                                .unwrap_or(false)
                            {
                                seg_files.push(seg_out_path);
                            }
                        }

                        if seg_files.len() == segments.len() {
                            let list_path = temp_dir.join("list.txt");
                            let mut list_content = String::new();
                            for f in &seg_files {
                                list_content.push_str(&format!("file '{}'\n", f.to_string_lossy()));
                            }
                            let _ = std::fs::write(&list_path, list_content);
                            let concat_out = temp_dir.join("concat_v.mp4");
                            let mut concat_cmd = Command::new(resolve_binary("ffmpeg"));
                            concat_cmd.args([
                                "-nostdin",
                                "-y",
                                "-f",
                                "concat",
                                "-safe",
                                "0",
                                "-i",
                                &list_path.to_string_lossy(),
                                "-c",
                                "copy",
                                &concat_out.to_string_lossy(),
                            ]);
                            if concat_cmd
                                .output()
                                .map(|o| o.status.success())
                                .unwrap_or(false)
                            {
                                let mut final_vf = Vec::new();
                                if plan.punch_zoom {
                                    final_vf.push(crate::pro_editor::build_punch_zoom_filter(
                                        1080, 1920,
                                    ));
                                }
                                if let Some(CaptionPlan::AssSubtitle(ass)) = &plan.captions {
                                    if let Some(ass_filter) = build_ass_filter(ass) {
                                        final_vf.push(ass_filter);
                                    }
                                } else if let Some(CaptionPlan::Drawtext(drawtext)) = &plan.captions
                                {
                                    if !drawtext.is_empty() {
                                        final_vf.push(drawtext.to_string());
                                    }
                                }

                                let mut final_cmd = Command::new(resolve_binary("ffmpeg"));
                                final_cmd.arg("-nostdin");
                                final_cmd.args(["-y", "-i", &concat_out.to_string_lossy()]);
                                final_cmd.args([
                                    "-ss",
                                    &start,
                                    "-t",
                                    &duration,
                                    "-i",
                                    &plan.source,
                                ]);
                                final_cmd.args(["-map", "0:v", "-map", "1:a?"]);

                                if !final_vf.is_empty() {
                                    final_cmd.args(["-vf", &final_vf.join(",")]);
                                    apply_video_encoder_args(
                                        &mut final_cmd,
                                        use_videotoolbox,
                                        plan.output.video_bitrate_kbps,
                                    );
                                } else {
                                    final_cmd.args(["-c:v", "copy"]);
                                }

                                let mut audio_filters = Vec::new();
                                if let Some((_, ref a_jump)) = jump_cuts {
                                    audio_filters.push(a_jump.clone());
                                }
                                if plan.audio.studio_audio {
                                    audio_filters.push(build_studio_audio_filter());
                                }
                                if !audio_filters.is_empty() {
                                    final_cmd.args([
                                        "-af",
                                        &audio_filters.join(","),
                                        "-c:a",
                                        "aac",
                                        "-b:a",
                                        &format!("{}k", plan.output.audio_bitrate_kbps),
                                    ]);
                                } else {
                                    final_cmd.args([
                                        "-c:a",
                                        "aac",
                                        "-b:a",
                                        &format!("{}k", plan.output.audio_bitrate_kbps),
                                    ]);
                                }
                                final_cmd.arg(&plan.output_path);

                                let res = final_cmd.output();
                                let _ = std::fs::remove_dir_all(&temp_dir);
                                if let Ok(out) = res {
                                    if out.status.success() {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        let _ = std::fs::remove_dir_all(&temp_dir);
                    }

                    // Single-pass render
                    let layout_type = segments
                        .first()
                        .map(|s| s.layout_type.as_str())
                        .unwrap_or("split_two");
                    let assigned_ids = segments
                        .first()
                        .map(|s| s.person_ids.as_slice())
                        .unwrap_or(&[1, 2]);

                    let base_filter = build_segment_filter_graph(
                        layout_type,
                        assigned_ids,
                        people,
                        iw_f,
                        ih_f,
                        0.0,
                        duration_sec,
                        top_fallback_x,
                        top_fallback_y,
                        bot_fallback_x,
                        bot_fallback_y,
                    );
                    let mut filter_graph = format!("{}[divided]", base_filter);

                    let pre_sub_stream = if plan.punch_zoom {
                        filter_graph = format!(
                            "{};[divided]{}[punched]",
                            filter_graph,
                            crate::pro_editor::build_punch_zoom_filter(1080, 1920)
                        );
                        "[punched]"
                    } else {
                        "[divided]"
                    };

                    let sub_out = if let Some(CaptionPlan::AssSubtitle(ass)) = &plan.captions {
                        if let Some(ass_filter) = build_ass_filter(ass) {
                            filter_graph = format!("{};{}[v_sub]", filter_graph, ass_filter);
                            "[v_sub]"
                        } else {
                            pre_sub_stream
                        }
                    } else if let Some(CaptionPlan::Drawtext(drawtext)) = &plan.captions {
                        if !drawtext.is_empty() {
                            filter_graph =
                                format!("{};{}{}[v_draw]", filter_graph, pre_sub_stream, drawtext);
                            "[v_draw]"
                        } else {
                            pre_sub_stream
                        }
                    } else {
                        pre_sub_stream
                    };

                    if let Some((ref v_jump, _)) = jump_cuts {
                        filter_graph = format!("{};{}{}[v_out]", filter_graph, sub_out, v_jump);
                    } else {
                        filter_graph = format!("{};{}null[v_out]", filter_graph, sub_out);
                    }

                    cmd.args([
                        "-filter_complex",
                        &filter_graph,
                        "-map",
                        "[v_out]",
                        "-map",
                        "0:a?",
                    ]);
                }
                ReframePlan::SmartFaceTrack => {
                    let center_x = detect_face_center_x(&plan.source, start_sec, duration_sec);
                    let mut filter = build_smart_face_crop_filter(center_x);
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

        let output = cmd.output().context("running ffmpeg clip render")?;
        if !output.status.success() {
            return Err(anyhow!(
                "ffmpeg clip render failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    };

    let vt_available = supports_videotoolbox();
    if vt_available {
        if let Err(vt_err) = run_render(true) {
            eprintln!(
                "VideoToolbox hardware render failed: {vt_err}. Retrying with software libx264..."
            );
            run_render(false)?;
        }
    } else {
        run_render(false)?;
    }

    Ok(plan.output_path.clone())
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
    let mode = reframe_mode.unwrap_or("vertical_crop");
    let effective_punch = punch_zoom || mode == "punch_zoom";

    let captions = if let Some(ass) = ass_subtitle_path {
        Some(CaptionPlan::AssSubtitle(ass.to_path_buf()))
    } else if let Some(drawtext) = drawtext_filters {
        Some(CaptionPlan::Drawtext(drawtext.to_string()))
    } else {
        None
    };

    let plan = RenderPlan::single_clip(
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
        OutputPreset::instagram_reels(),
    );

    execute_render_plan(&plan)
}
