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

pub fn render_flat_clip(
    source_path: &str,
    start_sec: f64,
    end_sec: f64,
    output_path: &Path,
    drawtext_filters: Option<&str>,
    reframe_mode: Option<&str>,
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

    let run_render = |use_videotoolbox: bool| -> Result<()> {
        let mut cmd = Command::new(resolve_binary("ffmpeg"));
        cmd.args(["-y", "-ss", &start, "-i", source_path, "-t", &duration]);

        if has_video {
            match mode {
                "original" => {
                    let mut filter = "scale='2*trunc(iw/2)':'2*trunc(ih/2)'".to_string();
                    if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    cmd.args(["-vf", &filter]);
                }
                "vertical_crop" => {
                    let mut filter = "crop=w='2*trunc(min(iw,ih*9/16)/2)':h='2*trunc(min(ih,iw*16/9)/2)',scale=1080:1920".to_string();
                    if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    cmd.args(["-vf", &filter]);
                }
                "podcast_split" => {
                    // Feature 1: Host & Guest Stacked Split-Screen for Podcasts & Interviews
                    let mut filter_graph = "[0:v]crop=iw/2:ih:0:0,scale=1080:960[top];[0:v]crop=iw/2:ih:iw/2:0,scale=1080:960[bot];[top][bot]vstack[stacked]".to_string();
                    if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter_graph = format!("{};[stacked]{}[v_out]", filter_graph, drawtext);
                        } else {
                            filter_graph = format!("{};[stacked]null[v_out]", filter_graph);
                        }
                    } else {
                        filter_graph = format!("{};[stacked]null[v_out]", filter_graph);
                    }
                    cmd.args(["-filter_complex", &filter_graph, "-map", "[v_out]", "-map", "0:a?"]);
                }
                "punch_zoom" => {
                    // Feature 1: Attention Retention Zoom Cuts (Subtle 1.12x punch zoom cut every 5.5s)
                    let mut filter = "scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,crop=w='iw/if(lt(mod(t,5.5),1.6),1.14,1.0)':h='ih/if(lt(mod(t,5.5),1.6),1.14,1.0)':x='(iw-ow)/2':y='(ih-oh)/2',scale=1080:1920".to_string();
                    if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
                    }
                    cmd.args(["-vf", &filter]);
                }
                _ => {
                    // "vertical_crop" (9:16 Center Crop)
                    let mut filter = "crop=w='2*trunc(min(iw,ih*9/16)/2)':h='2*trunc(min(ih,iw*16/9)/2)',scale=1080:1920".to_string();
                    if let Some(drawtext) = drawtext_filters {
                        if !drawtext.is_empty() {
                            filter = format!("{},{}", filter, drawtext);
                        }
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

        cmd.args(["-af", "loudnorm=I=-14:TP=-1.5:LRA=11,afftdn=nf=-25", "-c:a", "aac", "-b:a", "192k"]);
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
