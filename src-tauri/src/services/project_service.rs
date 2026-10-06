use std::path::PathBuf;
use anyhow::{anyhow, Result};

use crate::analysis_cache;
use crate::media;
use crate::models::{MediaProbe, Project, ProjectDetail};
use crate::AppState;

#[derive(serde::Serialize)]
pub struct DefaultFolders {
    pub youtube_download_dir: String,
    pub clips_output_dir: String,
}

#[derive(serde::Serialize)]
pub struct CopyrightCheckResult {
    pub is_safe: bool,
    pub license: Option<String>,
}

pub fn project_dir(state: &AppState, project_id: &str) -> PathBuf {
    state.data_dir.join("projects").join(project_id)
}

pub fn documents_project_dir(project: &Project, custom_dir: Option<&str>) -> Result<PathBuf, String> {
    let base_dir = if let Some(dir) = custom_dir.filter(|d| !d.trim().is_empty()) {
        let trimmed = dir.trim();
        if trimmed.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&trimmed[2..])
            } else {
                PathBuf::from(trimmed)
            }
        } else {
            PathBuf::from(trimmed)
        }
    } else if let Ok(env_dir) =
        std::env::var("CLIPON_CLIPS_DIR").or_else(|_| std::env::var("AUTOSHORTS_CLIPS_DIR"))
    {
        let trimmed = env_dir.trim().to_string();
        if trimmed.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&trimmed[2..])
            } else {
                PathBuf::from(trimmed)
            }
        } else {
            PathBuf::from(trimmed)
        }
    } else {
        dirs::document_dir()
            .ok_or_else(|| "Could not find your Documents folder for clip output.".to_string())?
            .join("ClipOn")
    };

    Ok(base_dir.join(project_output_slug(project)))
}

pub fn project_output_slug(project: &Project) -> String {
    let stem = std::path::Path::new(&project.source_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&project.id);
    let slug = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    let id_short = if project.id.len() >= 8 {
        &project.id[..8]
    } else {
        &project.id
    };

    if slug.is_empty() {
        project.id.clone()
    } else {
        format!("{slug}-{id_short}")
    }
}

pub fn validate_media_extension(path: &str) -> Result<()> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .ok_or_else(|| anyhow!("Selected file does not have an extension"))?;

    let allowed = ["mp4", "mov", "mp3", "wav", "m4a"];
    if allowed.contains(&extension.as_str()) {
        Ok(())
    } else {
        Err(anyhow!(
            "Unsupported file type .{extension}. Use mp4, mov, mp3, wav, or m4a."
        ))
    }
}

pub fn extract_youtube_video_id(raw_input: &str) -> Result<String, String> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return Err("YouTube URL cannot be empty".to_string());
    }

    let is_valid_id = |s: &str| -> bool {
        s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };

    if is_valid_id(trimmed) {
        return Ok(trimmed.to_string());
    }

    let url_str = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    };

    let parsed = url::Url::parse(&url_str)
        .map_err(|_| "Invalid YouTube URL format".to_string())?;

    let host = parsed.host_str().unwrap_or("").to_lowercase();
    let is_valid_host = host == "youtube.com"
        || host == "www.youtube.com"
        || host == "m.youtube.com"
        || host == "music.youtube.com"
        || host == "youtu.be";

    if !is_valid_host {
        return Err("Only YouTube URLs (youtube.com or youtu.be) are supported for security and compliance.".to_string());
    }

    let id = if host == "youtu.be" {
        parsed
            .path_segments()
            .and_then(|mut segs| segs.next())
            .filter(|s| is_valid_id(s))
            .map(ToString::to_string)
    } else {
        let path = parsed.path();
        if path == "/watch" {
            parsed
                .query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.to_string())
                .filter(|s| is_valid_id(s))
        } else if path.starts_with("/shorts/") {
            path.strip_prefix("/shorts/")
                .and_then(|s| s.split('/').next())
                .filter(|s| is_valid_id(s))
                .map(ToString::to_string)
        } else if path.starts_with("/embed/") {
            path.strip_prefix("/embed/")
                .and_then(|s| s.split('/').next())
                .filter(|s| is_valid_id(s))
                .map(ToString::to_string)
        } else if path.starts_with("/v/") {
            path.strip_prefix("/v/")
                .and_then(|s| s.split('/').next())
                .filter(|s| is_valid_id(s))
                .map(ToString::to_string)
        } else {
            None
        }
    };

    id.ok_or_else(|| "Could not extract a valid 11-character YouTube video ID from the provided URL.".to_string())
}

pub fn canonicalize_youtube_url(raw_input: &str) -> Result<String, String> {
    let video_id = extract_youtube_video_id(raw_input)?;
    Ok(format!("https://www.youtube.com/watch?v={}", video_id))
}

pub fn create_project_from_path(
    state: &AppState,
    path: &str,
    transcription_mode: &str,
    caption_style: &str,
) -> Result<Project, String> {
    validate_media_extension(path).map_err(|e| e.to_string())?;
    let probe = media::probe_media(path).ok();

    state
        .db
        .create_project(
            path,
            transcription_mode,
            caption_style,
            probe.and_then(|probe| probe.duration_sec),
        )
        .map_err(|e| e.to_string())
}

pub fn list_projects(state: &AppState) -> Result<Vec<Project>, String> {
    state.db.list_projects().map_err(|e| e.to_string())
}

pub fn get_project_detail(state: &AppState, project_id: &str) -> Result<ProjectDetail, String> {
    state.db.project_detail(project_id).map_err(|e| e.to_string())
}

pub fn probe_project(state: &AppState, project_id: &str) -> Result<MediaProbe, String> {
    let project = state.db.get_project(project_id).map_err(|e| e.to_string())?;
    let probe = media::probe_media(&project.source_path).map_err(|e| e.to_string())?;
    state
        .db
        .update_project_status(project_id, "ingest", probe.duration_sec)
        .map_err(|e| e.to_string())?;
    Ok(probe)
}

pub fn delete_project(state: &AppState, project_id: &str) -> Result<(), String> {
    state.db.delete_project(project_id).map_err(|e| e.to_string())?;

    let dir = project_dir(state, project_id);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            eprintln!(
                "Warning: failed to remove project directory {}: {}",
                dir.display(),
                e
            );
        }
    }

    Ok(())
}

pub fn rename_project(state: &AppState, project_id: &str, name: &str) -> Result<(), String> {
    state.db.rename_project(project_id, name).map_err(|e| e.to_string())
}

pub fn open_folder(path: &str) -> Result<(), String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("No path provided to open.".to_string());
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        #[cfg(target_os = "macos")]
        std::process::Command::new("open")
            .arg(trimmed)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
        #[cfg(target_os = "windows")]
        std::process::Command::new("explorer")
            .arg(trimmed)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
        #[cfg(target_os = "linux")]
        std::process::Command::new("xdg-open")
            .arg(trimmed)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
        return Ok(());
    }

    let resolved_path = if trimmed.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            home.join(&trimmed[2..]).to_string_lossy().to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    let p = std::path::Path::new(&resolved_path);

    // If it is a file, reveal it in Finder or Explorer!
    if p.is_file() {
        #[cfg(target_os = "macos")]
        std::process::Command::new("open")
            .args(["-R", &resolved_path])
            .spawn()
            .map_err(|e| format!("Failed to reveal file in Finder: {}", e))?;
        #[cfg(target_os = "windows")]
        std::process::Command::new("explorer")
            .args(["/select,", &resolved_path])
            .spawn()
            .map_err(|e| format!("Failed to reveal file in Explorer: {}", e))?;
        #[cfg(target_os = "linux")]
        {
            if let Some(parent) = p.parent() {
                std::process::Command::new("xdg-open")
                    .arg(parent)
                    .spawn()
                    .map_err(|e| format!("Failed to open folder: {}", e))?;
            }
        }
        return Ok(());
    }

    // If directory does not exist yet, create it
    if !p.exists() {
        std::fs::create_dir_all(p).ok();
    }

    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(&resolved_path)
        .spawn()
        .map_err(|e| format!("Failed to open folder in Finder: {}", e))?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("explorer")
        .arg(&resolved_path)
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open")
        .arg(&resolved_path)
        .spawn()
        .map_err(|e| format!("Failed to open folder: {}", e))?;

    Ok(())
}

pub fn get_default_folders() -> Result<DefaultFolders, String> {
    let youtube_download_dir = std::env::var("CLIPON_YOUTUBE_DIR")
        .or_else(|_| std::env::var("AUTOSHORTS_YOUTUBE_DIR"))
        .ok()
        .or_else(|| dirs::download_dir().map(|d| d.join("ClipOn").to_string_lossy().to_string()))
        .unwrap_or_else(|| "~/Downloads/ClipOn".to_string());

    let clips_output_dir = std::env::var("CLIPON_CLIPS_DIR")
        .or_else(|_| std::env::var("AUTOSHORTS_CLIPS_DIR"))
        .ok()
        .or_else(|| dirs::document_dir().map(|d| d.join("ClipOn").to_string_lossy().to_string()))
        .unwrap_or_else(|| "~/Documents/ClipOn".to_string());

    Ok(DefaultFolders {
        youtube_download_dir,
        clips_output_dir,
    })
}

pub async fn check_youtube_copyright(url: &str) -> Result<CopyrightCheckResult, String> {
    let canonical_url = canonicalize_youtube_url(url)?;

    tokio::task::spawn_blocking(move || {
        if !media::command_exists("yt-dlp") {
            return Err("yt-dlp is not installed or available on PATH. Please install via Homebrew: 'brew install yt-dlp'".to_string());
        }

        let output = std::process::Command::new(media::resolve_binary("yt-dlp"))
            .args(&["--dump-json", &canonical_url])
            .output()
            .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

        if !output.status.success() {
            return Err("Failed to fetch video metadata from YouTube.".to_string());
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|_| "Failed to parse yt-dlp output".to_string())?;

        let license = parsed
            .get("license")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let is_safe = if let Some(lic) = &license {
            lic.to_lowercase().contains("creative commons")
                || lic.to_lowercase().contains("reuse allowed")
        } else {
            false
        };

        Ok(CopyrightCheckResult { is_safe, license })
    })
    .await
    .map_err(|e| e.to_string())?
}

pub async fn download_youtube_video(
    url: &str,
    output_dir: Option<&str>,
    user_acknowledged: Option<bool>,
) -> Result<String, String> {
    if !user_acknowledged.unwrap_or(false) {
        return Err("Compliance policy enforcement: User confirmation of Terms of Service & legal rights is required before downloading.".to_string());
    }

    let canonical_url = canonicalize_youtube_url(url)?;
    let custom_output = output_dir.map(ToString::to_string);

    tokio::task::spawn_blocking(move || {
        if !media::command_exists("yt-dlp") {
            return Err("yt-dlp is not installed or available on PATH. Please install via Homebrew: 'brew install yt-dlp'".to_string());
        }

        let meta_output = std::process::Command::new(media::resolve_binary("yt-dlp"))
            .args(&["--dump-json", &canonical_url])
            .output()
            .map_err(|e| format!("Failed to run yt-dlp metadata check: {}", e))?;

        if !meta_output.status.success() {
            let stderr = String::from_utf8_lossy(&meta_output.stderr);
            return Err(format!("YouTube video is unavailable or metadata could not be retrieved: {}", stderr));
        }

        let save_dir = custom_output
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("CLIPON_YOUTUBE_DIR")
                    .or_else(|_| std::env::var("AUTOSHORTS_YOUTUBE_DIR"))
                    .ok()
            })
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::download_dir().map(|d| d.join("ClipOn")))
            .ok_or_else(|| "Could not find Downloads folder".to_string())?;

        std::fs::create_dir_all(&save_dir).ok();

        let output_template = save_dir.join("%(title)s_%(id)s.%(ext)s");
        let output_template_str = output_template.to_string_lossy().to_string();

        let output = std::process::Command::new(media::resolve_binary("yt-dlp"))
            .args(&[
                "--format",
                "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
                "--merge-output-format",
                "mp4",
                "-o",
                &output_template_str,
                "--print",
                "after_move:filepath",
                "--no-simulate",
                &canonical_url,
            ])
            .output()
            .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("yt-dlp download failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let filepath = stdout.lines().last().unwrap_or("").trim();

        if filepath.is_empty() || !std::path::Path::new(filepath).exists() {
            return Err("Could not locate downloaded file".to_string());
        }

        Ok(filepath.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn clear_all_storage(state: &AppState) -> Result<String, String> {
    state.db.clear_all().map_err(|e| e.to_string())?;

    let projects_dir = state.data_dir.join("projects");
    if projects_dir.exists() {
        let _ = std::fs::remove_dir_all(&projects_dir);
        let _ = std::fs::create_dir_all(&projects_dir);
    }

    if let Some(doc_dir) = dirs::document_dir() {
        let clipon_doc_dir = doc_dir.join("ClipOn");
        if clipon_doc_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&clipon_doc_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let is_clipon_generated = path.join("clips").exists()
                            || path.file_name().and_then(|n| n.to_str()).map_or(false, |name| {
                                name == "Clips" || name == "Youtube Video"
                            });
                        if is_clipon_generated {
                            let _ = std::fs::remove_dir_all(&path);
                        }
                    }
                }
            }
        }
    }

    if let Some(dl_dir) = dirs::download_dir() {
        let dl_clipon = dl_dir.join("ClipOn");
        if dl_clipon.exists() {
            if let Ok(entries) = std::fs::read_dir(&dl_clipon) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let is_media = path.extension().and_then(|e| e.to_str()).map_or(false, |ext| {
                        matches!(ext.to_ascii_lowercase().as_str(), "mp4" | "mkv" | "webm" | "part" | "ytdl")
                    });
                    if is_media {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }

    let cache = analysis_cache::AnalysisCache::global();
    let _ = cache.clear_all();

    Ok("All project storage, clips, analysis cache, and database records cleared successfully.".to_string())
}
