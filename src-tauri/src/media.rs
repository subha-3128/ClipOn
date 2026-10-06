use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::models::MediaProbe;

pub fn resolve_binary(name: &str) -> String {
    for prefix in &[
        "/opt/homebrew/bin",
        "/opt/homebrew/opt/ffmpeg-full/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
    ] {
        let p = Path::new(prefix).join(name);
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    name.to_string()
}

pub fn command_exists(name: &str) -> bool {
    let bin = resolve_binary(name);
    Command::new(bin).arg("-version").output().is_ok()
}

pub fn supports_videotoolbox() -> bool {
    #[cfg(target_os = "macos")]
    {
        let bin = resolve_binary("ffmpeg");
        if let Ok(output) = Command::new(bin).args(["-encoders"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.contains("h264_videotoolbox");
        }
    }
    false
}

pub fn probe_media(path: &str) -> Result<MediaProbe> {
    if !command_exists("ffprobe") {
        return Err(anyhow!("ffprobe is not installed or not available on PATH"));
    }

    let output = Command::new(resolve_binary("ffprobe"))
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .context("running ffprobe")?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let json: Value = serde_json::from_slice(&output.stdout).context("parsing ffprobe JSON")?;
    let streams = json
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let video = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    let audio = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));

    let duration_sec = json
        .get("format")
        .and_then(|format| format.get("duration"))
        .and_then(Value::as_str)
        .and_then(|duration| duration.parse::<f64>().ok());

    Ok(MediaProbe {
        duration_sec,
        has_video: video.is_some(),
        width: video
            .and_then(|stream| stream.get("width"))
            .and_then(Value::as_i64),
        height: video
            .and_then(|stream| stream.get("height"))
            .and_then(Value::as_i64),
        video_codec: video
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        audio_codec: audio
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

pub fn extract_audio(source_path: &str, project_dir: &Path) -> Result<PathBuf> {
    if !command_exists("ffmpeg") {
        return Err(anyhow!("ffmpeg is not installed or not available on PATH"));
    }

    std::fs::create_dir_all(project_dir)?;
    let output_path = project_dir.join("transcription_audio.wav");

    let output = Command::new(resolve_binary("ffmpeg"))
        .args(["-y", "-i", source_path, "-vn", "-ac", "1", "-ar", "16000"])
        .arg(&output_path)
        .output()
        .context("running ffmpeg audio extraction")?;

    if !output.status.success() {
        return Err(anyhow!(
            "ffmpeg audio extraction failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(output_path)
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PodcastFaceTracking {
    pub top_center_x: f64,
    pub top_center_y: f64,
    pub bottom_center_x: f64,
    pub bottom_center_y: f64,
    pub two_faces_detected: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FaceTrackerResult {
    pub avg_center_x: f64,
    pub face_detected: bool,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub podcast: Option<PodcastFaceTracking>,
}

pub fn detect_faces_full(source_path: &str, start_sec: f64, duration_sec: f64) -> FaceTrackerResult {
    let mut tracker_candidates = vec![
        "/Users/subhajitbepari/Desktop/AutoShorts/src-tauri/bin/clipon-face-tracker".to_string(),
        "/Applications/ClipOn.app/Contents/MacOS/clipon-face-tracker".to_string(),
        "/Applications/ClipOn.app/Contents/Resources/bin/clipon-face-tracker".to_string(),
    ];

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            tracker_candidates.push(parent.join("clipon-face-tracker").to_string_lossy().to_string());
            tracker_candidates.push(parent.join("../Resources/bin/clipon-face-tracker").to_string_lossy().to_string());
            tracker_candidates.push(parent.join("bin/clipon-face-tracker").to_string_lossy().to_string());
        }
    }

    let mut tracker_bin = None;
    for path in &tracker_candidates {
        if Path::new(path).exists() {
            tracker_bin = Some(path.clone());
            break;
        }
    }

    if let Some(bin) = tracker_bin {
        if let Ok(output) = Command::new(bin)
            .args([
                source_path,
                &format!("{start_sec:.3}"),
                &format!("{duration_sec:.3}"),
            ])
            .output()
        {
            if output.status.success() {
                if let Ok(res) = serde_json::from_slice::<FaceTrackerResult>(&output.stdout) {
                    return res;
                }
            }
        }
    }

    FaceTrackerResult {
        avg_center_x: 0.50,
        face_detected: false,
        width: None,
        height: None,
        podcast: Some(PodcastFaceTracking {
            top_center_x: 0.28,
            top_center_y: 0.36,
            bottom_center_x: 0.72,
            bottom_center_y: 0.36,
            two_faces_detected: false,
        }),
    }
}

/// Detect face horizontal center X in normalized coords [0.0, 1.0] using Apple Vision.
pub fn detect_face_center_x(source_path: &str, start_sec: f64, duration_sec: f64) -> f64 {
    detect_faces_full(source_path, start_sec, duration_sec)
        .avg_center_x
        .clamp(0.20, 0.80)
}

/// Detect silences longer than 0.40s using FFmpeg silencedetect.
pub fn detect_silences(source_path: &str, start_sec: f64, duration_sec: f64) -> Vec<(f64, f64)> {
    let start = format!("{start_sec:.3}");
    let duration = format!("{duration_sec:.3}");
    let output = Command::new(resolve_binary("ffmpeg"))
        .args([
            "-y",
            "-ss",
            &start,
            "-i",
            source_path,
            "-t",
            &duration,
            "-af",
            "silencedetect=noise=-30dB:d=0.45",
            "-f",
            "null",
            "-",
        ])
        .output();

    if let Ok(out) = output {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return crate::pro_editor::parse_silences_from_log(&stderr);
    }

    Vec::new()
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
) -> Result<PathBuf> {
    if !command_exists("ffmpeg") {
        return Err(anyhow!("ffmpeg is not installed or not available on PATH"));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let start = format!("{start_sec:.3}");
    let duration_sec = (end_sec - start_sec).max(0.1);
    let duration = format!("{duration_sec:.3}");

    let probe = probe_media(source_path).ok();
    let has_video = probe.as_ref().map(|p| p.has_video).unwrap_or(false);

    let mode = reframe_mode.unwrap_or("vertical_crop");
    let effective_punch = punch_zoom || mode == "punch_zoom";

    // Optional dead-air jump-cut filter
    let jump_cuts = if remove_silence {
        let silences = detect_silences(source_path, start_sec, duration_sec);
        crate::pro_editor::build_silence_jumpcut_filter(&silences, duration_sec)
    } else {
        None
    };

    let run_render = |use_videotoolbox: bool| -> Result<()> {
        let mut cmd = Command::new(resolve_binary("ffmpeg"));
        cmd.args(["-y", "-ss", &start, "-i", source_path, "-t", &duration]);

        if has_video {
            match mode {
                "original" => {
                    let iw_i = probe.as_ref().and_then(|p| p.width).unwrap_or(1920) as i64;
                    let ih_i = probe.as_ref().and_then(|p| p.height).unwrap_or(1080) as i64;
                    let mut filter = "scale='2*trunc(iw/2)':'2*trunc(ih/2)'".to_string();
                    if effective_punch {
                        filter = format!(
                            "{},{}",
                            filter,
                            crate::pro_editor::build_punch_zoom_filter(iw_i, ih_i)
                        );
                    }
                    if let Some(ass) = ass_subtitle_path {
                        if ass.exists() {
                            let escaped = ass
                                .to_string_lossy()
                                .replace('\\', "/")
                                .replace(':', "\\:")
                                .replace('\'', "'\\''");
                            filter = format!("{},ass='{}'", filter, escaped);
                        }
                    } else if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    if let Some((ref v_jump, _)) = jump_cuts {
                        filter = format!("{},{}", filter, v_jump);
                    }
                    cmd.args(["-vf", &filter]);
                }
                "smart_face_track" => {
                    // Feature 5: Native Apple Vision Face Tracking Crop
                    let center_x = detect_face_center_x(source_path, start_sec, duration_sec);
                    let crop_x = format!("min(max(0,({:.3}*iw-ow/2)),iw-ow)", center_x);
                    let mut filter = format!(
                        "crop=w='2*trunc(min(iw,ih*9/16)/2)':h='2*trunc(min(ih,iw*16/9)/2)':x='{}':y='(ih-oh)/2',scale=1080:1920",
                        crop_x
                    );
                    if effective_punch {
                        filter = format!(
                            "{},{}",
                            filter,
                            crate::pro_editor::build_punch_zoom_filter(1080, 1920)
                        );
                    }
                    if let Some(ass) = ass_subtitle_path {
                        if ass.exists() {
                            let escaped = ass
                                .to_string_lossy()
                                .replace('\\', "/")
                                .replace(':', "\\:")
                                .replace('\'', "'\\''");
                            filter = format!("{},ass='{}'", filter, escaped);
                        }
                    } else if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    if let Some((ref v_jump, _)) = jump_cuts {
                        filter = format!("{},{}", filter, v_jump);
                    }
                    cmd.args(["-vf", &filter]);
                }
                "podcast_split" => {
                    // Smart Two-Person Face-Tracked Podcast Split-Screen
                    let tracker_info = detect_faces_full(source_path, start_sec, duration_sec);
                    let iw_f = probe.as_ref().and_then(|p| p.width).unwrap_or(1920) as f64;
                    let ih_f = probe.as_ref().and_then(|p| p.height).unwrap_or(1080) as f64;

                    // 9:8 aspect ratio box for 1080x960
                    let mut cw = ((iw_f * 0.52).min(ih_f * 9.0 / 8.0) / 2.0).floor() * 2.0;
                    let mut ch = ((cw * 8.0 / 9.0) / 2.0).floor() * 2.0;
                    if ch > ih_f {
                        ch = (ih_f / 2.0).floor() * 2.0;
                        cw = ((ch * 9.0 / 8.0) / 2.0).floor() * 2.0;
                    }
                    let cw_i = cw as i64;
                    let ch_i = ch as i64;
                    let iw_i = iw_f as i64;
                    let ih_i = ih_f as i64;

                    let (top_x, top_y, bot_x, bot_y) = if let Some(ref pod) = tracker_info.podcast {
                        (pod.top_center_x, pod.top_center_y, pod.bottom_center_x, pod.bottom_center_y)
                    } else {
                        (0.28, 0.36, 0.72, 0.36)
                    };

                    // Top speaker crop (Left / Host): centered on face horizontally, upper 36% headroom vertically
                    let raw_x_top = ((top_x * iw_f) - (cw / 2.0)).round() as i64;
                    let mut x_top = raw_x_top.clamp(0, (iw_i - cw_i).max(0));
                    x_top -= x_top % 2;

                    let raw_y_top = ((top_y * ih_f) - (ch * 0.36)).round() as i64;
                    let mut y_top = raw_y_top.clamp(0, (ih_i - ch_i).max(0));
                    y_top -= y_top % 2;

                    // Bottom speaker crop (Right / Guest): centered on face horizontally, upper 36% headroom vertically
                    let raw_x_bot = ((bot_x * iw_f) - (cw / 2.0)).round() as i64;
                    let mut x_bot = raw_x_bot.clamp(0, (iw_i - cw_i).max(0));
                    x_bot -= x_bot % 2;

                    let raw_y_bot = ((bot_y * ih_f) - (ch * 0.36)).round() as i64;
                    let mut y_bot = raw_y_bot.clamp(0, (ih_i - ch_i).max(0));
                    y_bot -= y_bot % 2;

                    let mut filter_graph = format!(
                        "[0:v]crop={}:{}:{}:{},scale=1080:960[top];\
                         [0:v]crop={}:{}:{}:{},scale=1080:960[bot];\
                         [top][bot]vstack[stacked];\
                         [stacked]drawbox=x=0:y=956:w=1080:h=8:color=0x0a0d14@0.95:t=fill,\
                                  drawbox=x=0:y=958:w=1080:h=3:color=0x38bdf8@0.9:t=fill[divided]",
                        cw_i, ch_i, x_top, y_top,
                        cw_i, ch_i, x_bot, y_bot,
                    );

                    let pre_sub_stream = if effective_punch {
                        filter_graph = format!(
                            "{};[divided]{}[punched]",
                            filter_graph,
                            crate::pro_editor::build_punch_zoom_filter(1080, 1920)
                        );
                        "[punched]"
                    } else {
                        "[divided]"
                    };

                    if let Some(ass) = ass_subtitle_path {
                        if ass.exists() {
                            let escaped = ass
                                .to_string_lossy()
                                .replace('\\', "/")
                                .replace(':', "\\:")
                                .replace('\'', "'\\''");
                            filter_graph = format!("{};{}ass='{}'[v_sub]", filter_graph, pre_sub_stream, escaped);
                            if let Some((ref v_jump, _)) = jump_cuts {
                                filter_graph = format!("{};[v_sub]{}[v_out]", filter_graph, v_jump);
                            } else {
                                filter_graph = format!("{};[v_sub]null[v_out]", filter_graph);
                            }
                        } else {
                            filter_graph = format!("{};{}null[v_out]", filter_graph, pre_sub_stream);
                        }
                    } else if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter_graph = format!("{};{}{}[v_draw]", filter_graph, pre_sub_stream, drawtext);
                            if let Some((ref v_jump, _)) = jump_cuts {
                                filter_graph = format!("{};[v_draw]{}[v_out]", filter_graph, v_jump);
                            } else {
                                filter_graph = format!("{};[v_draw]null[v_out]", filter_graph);
                            }
                        } else {
                            filter_graph = format!("{};{}null[v_out]", filter_graph, pre_sub_stream);
                        }
                    } else {
                        if let Some((ref v_jump, _)) = jump_cuts {
                            filter_graph = format!("{};{}{}[v_out]", filter_graph, pre_sub_stream, v_jump);
                        } else {
                            filter_graph = format!("{};{}null[v_out]", filter_graph, pre_sub_stream);
                        }
                    }
                    cmd.args(["-filter_complex", &filter_graph, "-map", "[v_out]", "-map", "0:a?"]);
                }
                _ => {
                    // "vertical_crop" (9:16 Center Crop)
                    let mut filter = "crop=w='2*trunc(min(iw,ih*9/16)/2)':h='2*trunc(min(ih,iw*16/9)/2)',scale=1080:1920".to_string();
                    if effective_punch {
                        filter = format!(
                            "{},{}",
                            filter,
                            crate::pro_editor::build_punch_zoom_filter(1080, 1920)
                        );
                    }
                    if let Some(ass) = ass_subtitle_path {
                        if ass.exists() {
                            let escaped = ass
                                .to_string_lossy()
                                .replace('\\', "/")
                                .replace(':', "\\:")
                                .replace('\'', "'\\''");
                            filter = format!("{},ass='{}'", filter, escaped);
                        }
                    } else if let Some(drawtext) = drawtext_filters {
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

            if use_videotoolbox {
                cmd.args(["-c:v", "h264_videotoolbox", "-b:v", "6000k", "-pix_fmt", "yuv420p"]);
            } else {
                cmd.args(["-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p"]);
            }
        } else {
            cmd.arg("-vn");
        }

        let mut audio_filter = "loudnorm=I=-14:TP=-1.5:LRA=11,afftdn=nf=-25".to_string();
        if let Some((_, ref a_jump)) = jump_cuts {
            audio_filter = format!("{},{}", a_jump, audio_filter);
        }

        cmd.args(["-af", &audio_filter, "-c:a", "aac", "-b:a", "192k"]);
        cmd.arg(output_path);

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
            eprintln!("VideoToolbox hardware render failed: {vt_err}. Retrying with software libx264...");
            run_render(false)?;
        }
    } else {
        run_render(false)?;
    }

    Ok(output_path.to_path_buf())
}
