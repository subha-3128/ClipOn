use anyhow::{anyhow, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

use super::ffmpeg::{command_exists, resolve_binary};

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

pub fn detect_silences(source_path: &str, start_sec: f64, duration_sec: f64) -> Vec<(f64, f64)> {
    let cache = crate::analysis_cache::AnalysisCache::global();
    let cache_key = crate::analysis_cache::AnalysisCache::compute_source_key(
        source_path,
        start_sec,
        duration_sec,
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
