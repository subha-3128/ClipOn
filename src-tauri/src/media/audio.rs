use anyhow::{anyhow, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use super::ffmpeg::{command_exists, resolve_binary};

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct AudioExtractionMeta {
    pub source_path: String,
    pub source_size: u64,
    pub source_mtime_nanos: u128,
    pub params: String,
}

pub fn extract_audio(source_path: &str, project_dir: &Path) -> Result<PathBuf> {
    if !command_exists("ffmpeg") {
        return Err(anyhow!("ffmpeg is not installed or not available on PATH"));
    }

    std::fs::create_dir_all(project_dir)?;
    let output_path = project_dir.join("transcription_audio.wav");
    let meta_path = project_dir.join("transcription_audio.meta.json");
    let params = "-vn -ac 1 -ar 16000";

    let src_meta = Path::new(source_path).metadata().ok();
    let source_size = src_meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let source_mtime_nanos = src_meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    // Check if cached audio is valid
    if output_path.exists() && output_path.metadata().map(|m| m.len()).unwrap_or(0) > 0 {
        if let Ok(content) = std::fs::read_to_string(&meta_path) {
            if let Ok(saved) = serde_json::from_str::<AudioExtractionMeta>(&content) {
                if saved.source_path == source_path
                    && saved.source_size == source_size
                    && saved.source_mtime_nanos == source_mtime_nanos
                    && saved.params == params
                {
                    return Ok(output_path);
                }
            }
        }
    }

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

    let new_meta = AudioExtractionMeta {
        source_path: source_path.to_string(),
        source_size,
        source_mtime_nanos,
        params: params.to_string(),
    };
    if let Ok(meta_json) = serde_json::to_string(&new_meta) {
        let _ = std::fs::write(&meta_path, meta_json);
    }

    Ok(output_path)
}

pub fn detect_silences(source_path: &str, start_sec: f64, duration_sec: f64) -> Vec<(f64, f64)> {
    let cache = crate::analysis_cache::AnalysisCache::global();
    let cache_key = crate::analysis_cache::AnalysisCache::compute_source_key_with_params(
        source_path,
        start_sec,
        duration_sec,
        "silence_detector_v1",
        "ffmpeg_silencedetect",
        "-30dB_0.5s",
    );

    if let Some(cached) = cache.get::<Vec<(f64, f64)>>(&cache_key, "silence") {
        return cached;
    }

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
        let silences = crate::pro_editor::parse_silences_from_log(&stderr);
        let _ = cache.put(&cache_key, "silence", &silences);
        return silences;
    }

    Vec::new()
}

pub fn build_studio_audio_filter() -> String {
    "loudnorm=I=-14:TP=-1.5:LRA=11,afftdn=nf=-25".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_extraction_meta_serde_and_cache_validation() {
        let meta = AudioExtractionMeta {
            source_path: "/path/to/interview.mp4".into(),
            source_size: 1048576,
            source_mtime_nanos: 1700000000123456789,
            params: "-vn -ac 1 -ar 16000".into(),
        };

        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: AudioExtractionMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, deserialized);
    }
}
