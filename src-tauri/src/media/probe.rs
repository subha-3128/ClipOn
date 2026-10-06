use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::process::Command;

use super::ffmpeg::{command_exists, resolve_binary};
use crate::models::MediaProbe;

pub fn probe_media(path: &str) -> Result<MediaProbe> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(anyhow!("Media file does not exist: {}", path));
    }

    let meta = std::fs::metadata(p).context("reading media file metadata")?;
    if meta.len() == 0 {
        return Err(anyhow!("Media file is empty (0 bytes): {}", path));
    }

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
            "ffprobe failed to read media (file may be corrupted or unsupported): {}",
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
            .map(|s| s.to_string()),
        audio_codec: audio
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_probe_media_nonexistent_file() {
        let res = probe_media("/nonexistent/file_12345.mp4");
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_probe_media_zero_byte_file() {
        let tmp = std::env::temp_dir().join("zero_byte_test.mp4");
        std::fs::File::create(&tmp).unwrap();
        let res = probe_media(tmp.to_str().unwrap());
        let _ = std::fs::remove_file(&tmp);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("0 bytes"));
    }

    #[test]
    fn test_probe_media_corrupt_file() {
        let tmp = std::env::temp_dir().join("corrupt_test.mp4");
        let mut f = std::fs::File::create(&tmp).unwrap();
        f.write_all(b"not a valid video stream content here").unwrap();
        let res = probe_media(tmp.to_str().unwrap());
        let _ = std::fs::remove_file(&tmp);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("corrupted or unsupported"));
    }
}
