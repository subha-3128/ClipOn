mod db;
mod instagram;
mod llm;
mod media;
mod models;
mod transcription;

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use tauri::{Emitter, Manager};

use db::Database;
use models::{
    Candidate, EnvironmentStatus, MediaProbe, NormalizedTranscript, Project, ProjectDetail,
    Transcript, TranscriptWord,
};

#[derive(Clone, serde::Serialize)]
struct PullProgressPayload {
    status: String,
    completed: Option<u64>,
    total: Option<u64>,
    percentage: Option<f64>,
}

#[derive(Clone)]
struct AppState {
    db: Database,
    data_dir: PathBuf,
}

#[tauri::command]
async fn environment_status(state: tauri::State<'_, AppState>) -> Result<EnvironmentStatus, String> {
    let env_file = state.data_dir.join(".env");
    if env_file.exists() {
        let _ = dotenvy::from_path(&env_file);
    }
    let llm_provider = std::env::var("LLM_PROVIDER")
        .unwrap_or_else(|_| "deepseek".to_string())
        .to_lowercase();

    let has_local_whisper_model = transcription::whisper_cli_exists() || transcription::whisper_python_exists();

    let has_ollama = reqwest::Client::new()
        .get("http://localhost:11434")
        .timeout(std::time::Duration::from_millis(1000))
        .send()
        .await
        .is_ok();

    Ok(EnvironmentStatus {
        data_dir: state.data_dir.to_string_lossy().to_string(),
        has_ffmpeg: media::command_exists("ffmpeg"),
        has_ffprobe: media::command_exists("ffprobe"),
        has_deepgram_key: std::env::var("DEEPGRAM_API_KEY").is_ok(),
        has_anthropic_key: std::env::var("ANTHROPIC_API_KEY").is_ok(),
        has_deepseek_key: std::env::var("DEEPSEEK_API_KEY").is_ok(),
        has_gemini_key: std::env::var("GEMINI_API_KEY").is_ok(),
        has_openai_key: std::env::var("OPENAI_API_KEY").is_ok(),
        has_openrouter_key: std::env::var("OPENROUTER_API_KEY").is_ok(),
        has_groq_key: std::env::var("GROQ_API_KEY").is_ok(),
        llm_provider,
        has_local_whisper_model,
        has_ollama,
        has_ytdlp: media::command_exists("yt-dlp"),
        has_hardware_accel: media::supports_videotoolbox(),
        deepgram_key: std::env::var("DEEPGRAM_API_KEY").ok(),
        gemini_key: std::env::var("GEMINI_API_KEY").ok(),
        deepseek_key: std::env::var("DEEPSEEK_API_KEY").ok(),
        anthropic_key: std::env::var("ANTHROPIC_API_KEY").ok(),
        groq_key: std::env::var("GROQ_API_KEY").ok(),
        openai_key: std::env::var("OPENAI_API_KEY").ok(),
        openrouter_key: std::env::var("OPENROUTER_API_KEY").ok(),
        instagram_account_id: std::env::var("INSTAGRAM_ACCOUNT_ID").ok(),
        instagram_access_token: std::env::var("INSTAGRAM_ACCESS_TOKEN").ok(),
    })
}

#[tauri::command]
async fn pull_ollama_model(
    app: tauri::AppHandle,
    model_name: String,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    
    let mut response = client
        .post("http://localhost:11434/api/pull")
        .json(&serde_json::json!({
            "name": model_name,
            "stream": true,
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama pull returned status {status}: {text}"));
    }

    let mut buffer = String::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

        // Process lines in buffer
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(err_msg) = val.get("error").and_then(|v| v.as_str()) {
                    return Err(err_msg.to_string());
                }

                let completed = val.get("completed").and_then(|v| v.as_u64());
                let total = val.get("total").and_then(|v| v.as_u64());

                let mut status = val.get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Downloading...")
                    .to_string();

                if status.starts_with("downloading ") {
                    if let (Some(c), Some(t)) = (completed, total) {
                        let c_mb = c as f64 / 1024.0 / 1024.0;
                        let t_mb = t as f64 / 1024.0 / 1024.0;
                        if t_mb > 100.0 {
                            status = format!("Downloading weights: {:.1} MB / {:.1} MB", c_mb, t_mb);
                        } else {
                            status = format!("Downloading model components: {:.1} MB / {:.1} MB", c_mb, t_mb);
                        }
                    } else {
                        status = "Downloading model components...".to_string();
                    }
                }
                
                let percentage = if let (Some(c), Some(t)) = (completed, total) {
                    if t > 0 {
                        Some((c as f64 / t as f64) * 100.0)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let payload = PullProgressPayload {
                    status,
                    completed,
                    total,
                    percentage,
                };

                let _ = app.emit("ollama-pull-progress", payload);
            }
        }
    }

    Ok(())
}

#[tauri::command]
async fn install_ollama(app: tauri::AppHandle) -> Result<(), String> {
    let _ = app.emit("ollama-install-status", "Checking if Ollama is already installed...");
    let launch = std::process::Command::new("open")
        .args(["-a", "Ollama"])
        .output();
    
    if let Ok(out) = launch {
        if out.status.success() {
            let _ = app.emit("ollama-install-status", "Ollama is installed. Launching...");
            for _ in 0..12 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if reqwest::Client::new().get("http://localhost:11434").send().await.is_ok() {
                    let _ = app.emit("ollama-install-status", "Ollama started successfully!");
                    return Ok(());
                }
            }
        }
    }

    let brew_path = if std::path::Path::new("/opt/homebrew/bin/brew").exists() {
        Some("/opt/homebrew/bin/brew")
    } else if std::path::Path::new("/usr/local/bin/brew").exists() {
        Some("/usr/local/bin/brew")
    } else {
        None
    };

    if let Some(path) = brew_path {
        let _ = app.emit("ollama-install-status", "Installing Ollama via Homebrew Cask...");
        
        let output = std::process::Command::new(path)
            .args(["install", "--cask", "ollama"])
            .output()
            .map_err(|e| format!("Failed to run brew command: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if !stderr.contains("already installed") {
                return Err(format!("Brew install failed: {}", stderr));
            }
        }

        let _ = app.emit("ollama-install-status", "Starting Ollama.app...");
        let launch = std::process::Command::new("open")
            .args(["-a", "Ollama"])
            .output();
        
        if let Ok(out) = launch {
            if out.status.success() {
                for _ in 0..12 {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    if reqwest::Client::new().get("http://localhost:11434").send().await.is_ok() {
                        let _ = app.emit("ollama-install-status", "Ollama started successfully!");
                        return Ok(());
                    }
                }
            }
        }
    }

    let _ = app.emit("ollama-install-status", "Downloading Ollama zip from official source...");
    let temp_dir = std::env::temp_dir();
    let zip_path = temp_dir.join("Ollama-darwin.zip");
    
    let response = reqwest::get("https://ollama.com/download/Ollama-darwin.zip")
        .await
        .map_err(|e| format!("Failed to download Ollama: {e}"))?;

    let bytes = response.bytes().await.map_err(|e| format!("Failed to read Ollama bytes: {e}"))?;
    std::fs::write(&zip_path, bytes).map_err(|e| format!("Failed to save Ollama zip: {e}"))?;

    let _ = app.emit("ollama-install-status", "Unzipping Ollama package...");
    let unzip_output = std::process::Command::new("unzip")
        .args(["-o", &zip_path.to_string_lossy().to_string(), "-d", &temp_dir.to_string_lossy().to_string()])
        .output()
        .map_err(|e| format!("Failed to unzip Ollama: {e}"))?;

    if !unzip_output.status.success() {
        return Err(format!("Failed to unzip: {}", String::from_utf8_lossy(&unzip_output.stderr)));
    }

    let _ = app.emit("ollama-install-status", "Installing to Applications folder...");
    let app_src = temp_dir.join("Ollama.app");
    
    let mv_output = std::process::Command::new("mv")
        .args([&app_src.to_string_lossy().to_string(), "/Applications/"])
        .output()
        .map_err(|e| format!("Failed to move Ollama to Applications: {e}"))?;

    if !mv_output.status.success() {
        let user_apps = dirs::home_dir()
            .ok_or_else(|| "Could not find home directory".to_string())?
            .join("Applications");
        std::fs::create_dir_all(&user_apps).map_err(|e| format!("Failed to create ~/Applications: {e}"))?;
        
        let mv_user_output = std::process::Command::new("mv")
            .args([&app_src.to_string_lossy().to_string(), &user_apps.to_string_lossy().to_string()])
            .output()
            .map_err(|e| format!("Failed to move Ollama to ~/Applications: {e}"))?;

        if !mv_user_output.status.success() {
            return Err(format!("Failed to install Ollama to Applications folder: {}", String::from_utf8_lossy(&mv_user_output.stderr)));
        }
    }

    let _ = app.emit("ollama-install-status", "Starting Ollama...");
    let launch = std::process::Command::new("open")
        .args(["-a", "Ollama"])
        .output();

    if launch.is_ok() {
        for _ in 0..12 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if reqwest::Client::new().get("http://localhost:11434").send().await.is_ok() {
                let _ = app.emit("ollama-install-status", "Ollama started successfully!");
                return Ok(());
            }
        }
    }

    Err("Ollama installed but could not be automatically started. Please open Ollama from your Applications folder.".to_string())
}

#[tauri::command]
fn create_project_from_path(
    state: tauri::State<'_, AppState>,
    path: String,
    transcription_mode: String,
    caption_style: String,
) -> Result<Project, String> {
    validate_media_extension(&path).map_err(to_command_error)?;
    let probe = media::probe_media(&path).ok();

    state
        .db
        .create_project(
            &path,
            &transcription_mode,
            &caption_style,
            probe.and_then(|probe| probe.duration_sec),
        )
        .map_err(to_command_error)
}

#[tauri::command]
fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<Project>, String> {
    state.db.list_projects().map_err(to_command_error)
}

#[tauri::command]
fn get_project_detail(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<ProjectDetail, String> {
    state
        .db
        .project_detail(&project_id)
        .map_err(to_command_error)
}

#[tauri::command]
fn probe_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<MediaProbe, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(to_command_error)?;
    let probe = media::probe_media(&project.source_path).map_err(to_command_error)?;
    state
        .db
        .update_project_status(&project_id, "ingest", probe.duration_sec)
        .map_err(to_command_error)?;
    Ok(probe)
}

#[tauri::command]
fn extract_project_audio(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<String, String> {
    let project = state
        .db
        .get_project(&project_id)
        .map_err(to_command_error)?;
    let audio_path = media::extract_audio(&project.source_path, &project_dir(&state, &project_id))
        .map_err(to_command_error)?;
    Ok(audio_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn transcribe_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
    provider: String,
    api_key: Option<String>,
) -> Result<Transcript, String> {
    let db = state.db.clone();
    let data_dir = state.data_dir.clone();
    let project = db.get_project(&project_id).map_err(to_command_error)?;
    db.update_project_status(&project_id, "transcribing", None)
        .map_err(to_command_error)?;

    let transcript = match provider.as_str() {
        "deepgram" => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .or_else(|| std::env::var("DEEPGRAM_API_KEY").ok())
                .ok_or_else(|| {
                    "Set DEEPGRAM_API_KEY or paste an API key to use cloud transcription."
                        .to_string()
                })?;
            let audio_path = media::extract_audio(
                &project.source_path,
                &data_dir.join("projects").join(&project_id),
            )
            .map_err(to_command_error)?;
            transcription::transcribe_deepgram(&audio_path.to_string_lossy(), &key)
                .await
                .map_err(to_command_error)?
        }
        "local" => {
            let has_whisper = transcription::whisper_cli_exists() || transcription::whisper_python_exists();
            if !has_whisper {
                return Err("Whisper is not installed. Please install it (e.g., via Homebrew 'brew install whisper-cli' or via Python 'pip3 install openai-whisper').".to_string());
            }
            let audio_path = media::extract_audio(
                &project.source_path,
                &data_dir.join("projects").join(&project_id),
            )
            .map_err(to_command_error)?;
            transcription::transcribe_local(&audio_path.to_string_lossy(), &data_dir.to_string_lossy())
                .await
                .map_err(to_command_error)?
        }
        other => return Err(format!("Unsupported transcription provider: {other}")),
    };

    let raw_json = serde_json::to_string_pretty(&transcript).map_err(to_command_error)?;
    let saved = db
        .save_transcript(
            &project_id,
            &provider,
            &raw_json,
            Some(&transcript.language),
        )
        .map_err(to_command_error)?;
    db.update_project_status(&project_id, "analyzing", Some(transcript.duration))
        .map_err(to_command_error)?;
    Ok(saved)
}

#[tauri::command]
fn save_demo_transcript(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<Transcript, String> {
    let transcript = demo_transcript();
    let raw_json = serde_json::to_string_pretty(&transcript).map_err(to_command_error)?;
    let saved = state
        .db
        .save_transcript(&project_id, "demo", &raw_json, Some(&transcript.language))
        .map_err(to_command_error)?;
    state
        .db
        .update_project_status(&project_id, "analyzing", Some(transcript.duration))
        .map_err(to_command_error)?;
    Ok(saved)
}

#[tauri::command]
async fn generate_candidates(
    state: tauri::State<'_, AppState>,
    project_id: String,
    api_key: Option<String>,
    provider: Option<String>,
    model_name: Option<String>,
    _allow_demo: bool,
) -> Result<Vec<Candidate>, String> {
    let db = state.db.clone();
    let transcript = db
        .latest_transcript(&project_id)
        .map_err(to_command_error)?
        .ok_or_else(|| "Transcribe the project before detecting moments.".to_string())?;
    let normalized: NormalizedTranscript =
        serde_json::from_str(&transcript.raw_json).map_err(to_command_error)?;

    let active_provider = provider
        .or_else(|| std::env::var("LLM_PROVIDER").ok())
        .unwrap_or_else(|| "deepseek".to_string())
        .to_lowercase();

    let drafts = match active_provider.as_str() {
        "claude" => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .ok_or_else(|| "Set ANTHROPIC_API_KEY or supply Claude API Key to generate candidates.".to_string())?;
            llm::detect_candidates_with_claude(&normalized, &key)
                .await
                .map_err(to_command_error)?
        }
        "local" | "ollama" => {
            let model = model_name
                .filter(|m| !m.trim().is_empty())
                .or_else(|| std::env::var("OLLAMA_MODEL").ok())
                .unwrap_or_else(|| "llama3.2".to_string());
            llm::detect_candidates_with_local_llm(&normalized, &model)
                .await
                .map_err(to_command_error)?
        }
        "gemini" => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                .ok_or_else(|| "Set GEMINI_API_KEY or supply Gemini API Key to generate candidates.".to_string())?;
            llm::detect_candidates_with_gemini(&normalized, &key)
                .await
                .map_err(to_command_error)?
        }
        "openai" => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .ok_or_else(|| "Set OPENAI_API_KEY or supply OpenAI API Key to generate candidates.".to_string())?;
            llm::detect_candidates_with_openai(&normalized, &key)
                .await
                .map_err(to_command_error)?
        }
        "openrouter" => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .ok_or_else(|| "Set OPENROUTER_API_KEY or supply OpenRouter API Key to generate candidates.".to_string())?;
            llm::detect_candidates_with_openrouter(&normalized, &key, model_name.as_deref())
                .await
                .map_err(to_command_error)?
        }
        "groq" => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .or_else(|| std::env::var("GROQ_API_KEY").ok())
                .ok_or_else(|| "Set GROQ_API_KEY or supply Groq API Key to generate candidates.".to_string())?;
            llm::detect_candidates_with_groq(&normalized, &key)
                .await
                .map_err(to_command_error)?
        }
        _ => {
            let key = api_key
                .filter(|k| !k.trim().is_empty())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .ok_or_else(|| "Set DEEPSEEK_API_KEY or supply DeepSeek API Key to generate candidates.".to_string())?;
            llm::detect_candidates_with_deepseek(&normalized, &key, model_name.as_deref())
                .await
                .map_err(to_command_error)?
        }
    };

    if drafts.is_empty() {
        return Err("No viable clip candidates were returned for this transcript.".to_string());
    }

    let candidates = db
        .replace_candidates(&project_id, &drafts)
        .map_err(to_command_error)?;
    db.update_project_status(&project_id, "ready", None)
        .map_err(to_command_error)?;
    Ok(candidates)
}

#[tauri::command]
fn set_selected_clip_count(
    state: tauri::State<'_, AppState>,
    project_id: String,
    count: usize,
) -> Result<Vec<Candidate>, String> {
    state
        .db
        .set_selected_clip_count(&project_id, count.clamp(0, 10))
        .map_err(to_command_error)
}

#[tauri::command]
async fn update_candidate_timing(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    start_sec: f64,
    end_sec: f64,
) -> Result<models::Candidate, String> {
    let db = state.db.clone();
    let valid_start = start_sec.max(0.0);
    let valid_end = end_sec.max(valid_start + 1.0);
    db.update_candidate_timing(&candidate_id, valid_start, valid_end)
        .map_err(to_command_error)?;
    let (candidate, _) = db.get_candidate_with_project(&candidate_id).map_err(to_command_error)?;
    Ok(candidate)
}

#[tauri::command]
async fn render_flat_clip_for_candidate(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    reframe_mode: Option<String>,
    output_dir: Option<String>,
) -> Result<String, String> {
    let db = state.db.clone();
    let data_dir = state.data_dir.clone();
    let mode = reframe_mode.clone();
    let out_dir = output_dir.clone();

    tokio::task::spawn_blocking(move || {
        let (candidate, project) = db
            .get_candidate_with_project(&candidate_id)
            .map_err(to_command_error)?;
        db
            .update_clip_for_candidate(&candidate_id, "cutting", None, None, None)
            .map_err(to_command_error)?;

        let output_path = documents_project_dir(&project, out_dir.as_deref())?
            .join("clips")
            .join(format!("clip-{:02}_flat.mp4", candidate.rank));

        let mut srt_path = None;
        let mut drawtext_filters = None;

        let probe = media::probe_media(&project.source_path).ok();
        let cropped_width = if let Some(p) = &probe {
            let iw = p.width.unwrap_or(1920) as f64;
            let ih = p.height.unwrap_or(1080) as f64;
            let w = (iw.min(ih * 9.0 / 16.0) / 2.0).floor() * 2.0;
            w as i64
        } else {
            1080
        };

        if let Ok(Some(transcript_record)) = db.latest_transcript(&project.id) {
            if let Ok(normalized) = serde_json::from_str::<NormalizedTranscript>(&transcript_record.raw_json) {
                let srt_content = generate_srt(&normalized.words, candidate.start_sec, candidate.end_sec);
                let clip_srt_path = data_dir.join("projects").join(&project.id).join(format!("clip-{}.srt", candidate.id));
                if std::fs::write(&clip_srt_path, srt_content).is_ok() {
                    srt_path = Some(clip_srt_path);
                }
                let style = project.caption_style.as_deref().unwrap_or("modern-box");
                let drawtext = build_drawtext_filters(
                    &normalized.words,
                    candidate.start_sec,
                    candidate.end_sec,
                    cropped_width,
                    style,
                );
                if !drawtext.is_empty() {
                    drawtext_filters = Some(drawtext);
                }
            }
        }

        match media::render_flat_clip(
            &project.source_path,
            candidate.start_sec,
            candidate.end_sec,
            &output_path,
            drawtext_filters.as_deref(),
            mode.as_deref(),
        ) {
            Ok(path) => {
                let path_string = path.to_string_lossy().to_string();
                let srt_string = srt_path.map(|p| p.to_string_lossy().to_string());
                db
                    .update_clip_for_candidate(
                        &candidate_id,
                        "done",
                        Some(&path_string),
                        srt_string.as_deref(),
                        None,
                    )
                    .map_err(to_command_error)?;
                Ok(path_string)
            }
            Err(error) => {
                let err_msg = error.to_string();
                // Fallback retry rendering without captions overlay on any error
                match media::render_flat_clip(
                    &project.source_path,
                    candidate.start_sec,
                    candidate.end_sec,
                    &output_path,
                    None,
                    mode.as_deref(),
                ) {
                    Ok(path) => {
                        let path_string = path.to_string_lossy().to_string();
                        let srt_string = srt_path.map(|p| p.to_string_lossy().to_string());
                        let warning_msg = format!(
                            "Clip rendered successfully, but captions were skipped. Error: {}",
                            err_msg
                        );
                        db
                            .update_clip_for_candidate(
                                &candidate_id,
                                "done",
                                Some(&path_string),
                                srt_string.as_deref(),
                                Some(&warning_msg),
                            )
                            .map_err(to_command_error)?;
                        Ok(path_string)
                    }
                    Err(retry_err) => {
                        let message = retry_err.to_string();
                        db
                            .update_clip_for_candidate(&candidate_id, "error", None, None, Some(&message))
                            .map_err(to_command_error)?;
                        Err(message)
                    }
                }
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn delete_project(state: tauri::State<'_, AppState>, project_id: String) -> Result<(), String> {
    state.db.delete_project(&project_id).map_err(to_command_error)
}

#[tauri::command]
fn rename_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
    name: String,
) -> Result<(), String> {
    state.db.rename_project(&project_id, &name).map_err(to_command_error)
}

/// Open a folder or reveal a file in the native file manager (Finder on macOS).
#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
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

#[derive(serde::Serialize)]
pub struct DefaultFolders {
    youtube_download_dir: String,
    clips_output_dir: String,
}

/// Returns the default save directories so the frontend can display them.
#[tauri::command]
fn get_default_folders() -> Result<DefaultFolders, String> {
    let youtube_download_dir = std::env::var("CLIPON_YOUTUBE_DIR")
        .or_else(|_| std::env::var("AUTOSHORTS_YOUTUBE_DIR"))
        .ok()
        .or_else(|| {
            dirs::download_dir().map(|d| d.join("ClipOn").to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "~/Downloads/ClipOn".to_string());

    let clips_output_dir = std::env::var("CLIPON_CLIPS_DIR")
        .or_else(|_| std::env::var("AUTOSHORTS_CLIPS_DIR"))
        .ok()
        .or_else(|| {
            dirs::document_dir().map(|d| d.join("ClipOn").to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "~/Documents/ClipOn".to_string());

    Ok(DefaultFolders { youtube_download_dir, clips_output_dir })
}


#[tauri::command]
async fn generate_social_kit_for_candidate(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
) -> Result<models::SocialKit, String> {
    let db = state.db.clone();
    let (candidate, project) = db
        .get_candidate_with_project(&candidate_id)
        .map_err(to_command_error)?;

    let mut transcript_text = String::new();
    if let Ok(Some(transcript_record)) = db.latest_transcript(&project.id) {
        if let Ok(normalized) = serde_json::from_str::<NormalizedTranscript>(&transcript_record.raw_json) {
            let words: Vec<&str> = normalized
                .words
                .iter()
                .filter(|w| w.end >= candidate.start_sec && w.start <= candidate.end_sec)
                .map(|w| w.text.as_str())
                .collect();
            transcript_text = words.join(" ");
        }
    }

    let kit = llm::generate_social_kit(&candidate.id, &candidate.hook, &transcript_text).await;
    Ok(kit)
}

pub fn run() {
    let _ = dotenvy::dotenv();

    let current_path = std::env::var("PATH").unwrap_or_default();
    let extended_path = format!("/opt/homebrew/bin:/opt/homebrew/opt/ffmpeg-full/bin:/usr/local/bin:/usr/bin:/bin:{}", current_path);
    std::env::set_var("PATH", extended_path);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .context("resolving app data directory")?;
            std::fs::create_dir_all(&data_dir).context("creating app data directory")?;
            std::fs::create_dir_all(data_dir.join("models")).context("creating models directory")?;
            let env_path = data_dir.join(".env");
            if !env_path.exists() {
                let project_env = std::path::PathBuf::from("/Users/subhajitbepari/Desktop/AutoShorts/.env");
                if project_env.exists() {
                    let _ = std::fs::copy(&project_env, &env_path);
                }
            }
            if env_path.exists() {
                let _ = dotenvy::from_path(&env_path);
            }
            let db_path = if data_dir.join("clipon.sqlite").exists() {
                data_dir.join("clipon.sqlite")
            } else if data_dir.join("autoshorts.sqlite").exists() {
                let _ = std::fs::copy(data_dir.join("autoshorts.sqlite"), data_dir.join("clipon.sqlite"));
                data_dir.join("clipon.sqlite")
            } else {
                data_dir.join("clipon.sqlite")
            };
            let db = Database::open(&db_path)?;
            app.manage(AppState { db, data_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            environment_status,
            pull_ollama_model,
            install_ollama,
            create_project_from_path,
            list_projects,
            get_project_detail,
            probe_project,
            extract_project_audio,
            transcribe_project,
            save_demo_transcript,
            generate_candidates,
            set_selected_clip_count,
            render_flat_clip_for_candidate,
            delete_project,
            rename_project,
            check_youtube_copyright,
            download_youtube_video,
            open_folder,
            get_default_folders,
            generate_social_kit_for_candidate,
            test_instagram_connection,
            publish_candidate_to_instagram,
            auto_publish_eligible_candidates,
            save_instagram_credentials,
            update_candidate_timing
        ])
        .run(tauri::generate_context!())
        .expect("error while running ClipOn");
}

#[derive(serde::Serialize)]
pub struct CopyrightCheckResult {
    is_safe: bool,
    license: Option<String>,
}

#[tauri::command]
async fn check_youtube_copyright(url: String) -> Result<CopyrightCheckResult, String> {
    tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new(media::resolve_binary("yt-dlp"))
            .args(&["--dump-json", &url])
            .output()
            .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

        if !output.status.success() {
            return Err("Failed to fetch video metadata from YouTube.".to_string());
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).map_err(|_| "Failed to parse yt-dlp output".to_string())?;

        let license = parsed.get("license").and_then(|v| v.as_str()).map(|s| s.to_string());
        
        let is_safe = if let Some(lic) = &license {
            lic.to_lowercase().contains("creative commons") || lic.to_lowercase().contains("reuse allowed")
        } else {
            false
        };

        Ok(CopyrightCheckResult { is_safe, license })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn download_youtube_video(url: String, output_dir: Option<String>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let save_dir = output_dir
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("CLIPON_YOUTUBE_DIR").or_else(|_| std::env::var("AUTOSHORTS_YOUTUBE_DIR")).ok())
            .map(std::path::PathBuf::from)
            .or_else(|| dirs::download_dir().map(|d| d.join("ClipOn")))
            .ok_or_else(|| "Could not find Downloads folder".to_string())?;

        std::fs::create_dir_all(&save_dir).ok();

        let output_template = save_dir.join("%(title)s_%(id)s.%(ext)s");
        let output_template_str = output_template.to_string_lossy().to_string();

        let output = std::process::Command::new(media::resolve_binary("yt-dlp"))
            .args(&[
                "--format", "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
                "--merge-output-format", "mp4",
                "-o", &output_template_str,
                "--print", "after_move:filepath",
                "--no-simulate",
                &url
            ])
            .output()
            .map_err(|e| format!("Failed to run yt-dlp: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("yt-dlp failed: {}", stderr));
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

fn project_dir(state: &AppState, project_id: &str) -> PathBuf {
    state.data_dir.join("projects").join(project_id)
}

fn documents_project_dir(project: &Project, custom_dir: Option<&str>) -> Result<PathBuf, String> {
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
    } else if let Ok(env_dir) = std::env::var("CLIPON_CLIPS_DIR").or_else(|_| std::env::var("AUTOSHORTS_CLIPS_DIR")) {
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

fn project_output_slug(project: &Project) -> String {
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

    if slug.is_empty() {
        project.id.clone()
    } else {
        slug
    }
}

fn validate_media_extension(path: &str) -> Result<()> {
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

fn demo_transcript() -> NormalizedTranscript {
    let lines = [
        "The surprising thing about short-form clips is that the best moment is rarely the loudest moment.",
        "It is usually the point where someone finally says the quiet part plainly and the listener can feel the stakes.",
        "That is why the system needs to understand the transcript as a story, not just search for keywords.",
        "A good clip opens with tension, resolves one idea, and ends before the energy leaks away.",
        "If you can rank those moments consistently, the rendering pipeline becomes much easier to trust.",
        "The creator still decides what represents them, but the machine removes the first exhausting pass through hours of footage.",
        "The goal is not to automate taste completely. The goal is to give taste a faster starting point.",
        "Once the strongest moments are visible, captions and platform copy become finishing work instead of discovery work.",
        "That is the workflow ClipOn is designed around.",
    ];

    let mut words = Vec::new();
    let mut cursor = 0.0;
    for line in lines {
        for token in line.split_whitespace() {
            let clean = token.to_string();
            let end = cursor + 0.32;
            words.push(TranscriptWord {
                text: clean,
                start: cursor,
                end,
                speaker: Some("A".to_string()),
            });
            cursor = end + 0.08;
        }
        cursor += 0.75;
    }

    let segments = transcription::build_segments(&words);

    NormalizedTranscript {
        language: "en".to_string(),
        duration: cursor,
        speakers: vec!["A".to_string()],
        words,
        segments,
    }
}

fn to_command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn generate_srt(words: &[TranscriptWord], start_sec: f64, end_sec: f64) -> String {
    let mut srt = String::new();
    let mut index = 1;

    let candidate_words: Vec<&TranscriptWord> = words
        .iter()
        .filter(|w| w.end > start_sec && w.start < end_sec)
        .collect();

    for chunk in candidate_words.chunks(3) {
        if chunk.is_empty() {
            continue;
        }
        let first = chunk[0];
        let last = chunk[chunk.len() - 1];

        let start_rel = (first.start - start_sec).max(0.0);
        let end_rel = (last.end - start_sec).min(end_sec - start_sec).max(0.0);

        let text = chunk
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        srt.push_str(&format!("{index}\n"));
        srt.push_str(&format!(
            "{}\n",
            format_srt_time(start_rel, end_rel)
        ));
        srt.push_str(&format!("{text}\n\n"));
        index += 1;
    }

    srt
}

fn format_srt_time(start: f64, end: f64) -> String {
    let format_time = |secs: f64| {
        let hours = (secs / 3600.0) as u32;
        let mins = ((secs % 3600.0) / 60.0) as u32;
        let secs_only = (secs % 60.0) as u32;
        let ms = ((secs.fract()) * 1000.0) as u32;
        format!("{hours:02}:{mins:02}:{secs_only:02},{ms:03}")
    };
    format!("{} --> {}", format_time(start), format_time(end))
}

fn get_contextual_emoji(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    if lower.contains("money") || lower.contains("dollar") || lower.contains("cash") || lower.contains("rich") || lower.contains("cost") || lower.contains("paid") || lower.contains("price") {
        Some("💰")
    } else if lower.contains("fire") || lower.contains("lit") || lower.contains("hot") || lower.contains("burn") {
        Some("🔥")
    } else if lower.contains("crazy") || lower.contains("mind") || lower.contains("insane") || lower.contains("shock") || lower.contains("unbelievable") {
        Some("🤯")
    } else if lower.contains("rocket") || lower.contains("fast") || lower.contains("speed") || lower.contains("growth") || lower.contains("explode") || lower.contains("scale") {
        Some("🚀")
    } else if lower.contains("laugh") || lower.contains("funny") || lower.contains("joke") || lower.contains("hilarious") || lower.contains("lol") {
        Some("😂")
    } else if lower.contains("party") || lower.contains("boat") || lower.contains("trip") || lower.contains("drink") || lower.contains("fun") || lower.contains("celebrat") {
        Some("🥳")
    } else if lower.contains("time") || lower.contains("late") || lower.contains("clock") || lower.contains("wait") || lower.contains("minute") || lower.contains("hour") || lower.contains("second") {
        Some("⏱️")
    } else if lower.contains("danger") || lower.contains("warn") || lower.contains("stop") || lower.contains("threat") || lower.contains("trouble") {
        Some("⚠️")
    } else if lower.contains("dead") || lower.contains("die") || lower.contains("kill") || lower.contains("skull") {
        Some("💀")
    } else if lower.contains("love") || lower.contains("heart") || lower.contains("best") || lower.contains("friend") {
        Some("❤️")
    } else if lower.contains("win") || lower.contains("won") || lower.contains("champ") || lower.contains("first") || lower.contains("trophy") {
        Some("🏆")
    } else if lower.contains("food") || lower.contains("eat") || lower.contains("dinner") || lower.contains("lunch") || lower.contains("cook") {
        Some("🍔")
    } else if lower.contains("look") || lower.contains("see") || lower.contains("watch") || lower.contains("eyes") {
        Some("👀")
    } else if lower.contains("secret") || lower.contains("quiet") || lower.contains("shh") {
        Some("🤫")
    } else {
        None
    }
}

fn build_drawtext_filters(
    words: &[TranscriptWord],
    start_sec: f64,
    end_sec: f64,
    cropped_width: i64,
    caption_style: &str,
) -> String {
    let candidate_words: Vec<&TranscriptWord> = words
        .iter()
        .filter(|w| w.end > start_sec && w.start < end_sec)
        .collect();

    if candidate_words.is_empty() {
        return String::new();
    }

    // Build font option once for drawtext filter
    let mut font_paths = vec![
        // macOS
        "/System/Library/Fonts/Supplemental/Futura.ttc".to_string(),
        "/System/Library/Fonts/Avenir Next.ttc".to_string(),
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf".to_string(),
        "/System/Library/Fonts/Helvetica.ttc".to_string(),
        // Windows standard
        "C:/Windows/Fonts/SegoeUIb.ttf".to_string(),
        "C:/Windows/Fonts/segoeuib.ttf".to_string(),
        "C:/Windows/Fonts/SegoeUI.ttf".to_string(),
        "C:/Windows/Fonts/segoeui.ttf".to_string(),
        "C:/Windows/Fonts/arialbd.ttf".to_string(),
        "C:/Windows/Fonts/arial.ttf".to_string(),
        // Linux
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf".to_string(),
        "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf".to_string(),
    ];
    if let Ok(windir) = std::env::var("WINDIR").or_else(|_| std::env::var("SystemRoot")) {
        let windir_fonts = format!("{}/Fonts", windir.replace('\\', "/"));
        font_paths.push(format!("{}/SegoeUIb.ttf", windir_fonts));
        font_paths.push(format!("{}/segoeuib.ttf", windir_fonts));
        font_paths.push(format!("{}/SegoeUI.ttf", windir_fonts));
        font_paths.push(format!("{}/arialbd.ttf", windir_fonts));
        font_paths.push(format!("{}/arial.ttf", windir_fonts));
    }

    let mut font_option = String::new();
    for path in &font_paths {
        if std::path::Path::new(path).exists() {
            let normalized_path = path.replace('\\', "/");
            let escaped_path = normalized_path.replace('\'', "'\\''");
            font_option = format!("fontfile='{}':", escaped_path);
            break;
        }
    }

    let mut drawtext_filters = Vec::new();

    // Group into chunks of 2 words for fast-paced style captions
    for chunk in candidate_words.chunks(2) {
        if chunk.is_empty() {
            continue;
        }
        let first = chunk[0];
        let last = chunk[chunk.len() - 1];

        // Timestamps relative to clip start (due to fast input seeking resetting stream PTS)
        let start_rel = (first.start - start_sec).max(0.0);
        let end_rel = (last.end - start_sec).min(end_sec - start_sec).max(0.0);
        if end_rel <= start_rel {
            continue;
        }

        let text = chunk
            .iter()
            .map(|w| w.text.to_uppercase())
            .collect::<Vec<_>>()
            .join(" ");

        // Clean text to avoid breaking filter parameters
        let clean_text: String = text.chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '!' || *c == '?')
            .collect();

        // Responsive font size and padding box
        let fontsize = ((cropped_width as f64) * 0.075).clamp(16.0, 80.0).round() as i64;
        let padding = ((fontsize as f64) * 0.3).clamp(4.0, 24.0).round() as i64;


        let emoji_suffix = get_contextual_emoji(&clean_text).unwrap_or("");
        let display_text = if !emoji_suffix.is_empty() {
            format!("{clean_text} {emoji_suffix}")
        } else {
            clean_text.clone()
        };

        let drawtext = match caption_style {
            "submagic-viral" => {
                // Feature 3: Submagic style - high-visibility vibrant yellow with dark glass pillbox and emojis
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.72:fontsize={}:fontcolor=0xFFE600:box=1:boxcolor=0x000000d0:boxborderw={}:shadowcolor=black@0.7:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, display_text, fontsize, padding, start_rel, end_rel
                )
            }
            "hormozi-punch" => {
                // Feature 3: Hormozi punch - heavy black text on vibrant yellow pill with auto-emojis
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.72:fontsize={}:fontcolor=black:box=1:boxcolor=0xFFE600f0:boxborderw={}:enable='between(t,{:.3},{:.3})'",
                    font_option, display_text, fontsize, padding, start_rel, end_rel
                )
            }
            "neon-glow" => {
                // Feature 3: Cyberpunk neon cyan with dark shadow & auto-emojis
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.7:fontsize={}:fontcolor=0x00FFFF:borderw=3:bordercolor=0x003366:shadowcolor=black@0.8:shadowx=3:shadowy=3:enable='between(t,{:.3},{:.3})'",
                    font_option, display_text, fontsize, start_rel, end_rel
                )
            }
            "classic-outline" => {
                // Classic yellow text with a bold outline (CapCut style)
                let borderw = ((fontsize as f64) * 0.1).clamp(2.0, 8.0).round() as i64;
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.65:fontsize={}:fontcolor=yellow:borderw={}:bordercolor=black:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, fontsize, borderw, start_rel, end_rel
                )
            }
            "minimal-shadow" => {
                // Sleek white text with a soft drop shadow (Minimalist)
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.7:fontsize={}:fontcolor=white:shadowcolor=black@0.5:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, fontsize, start_rel, end_rel
                )
            }
            "vibrant-cyan" => {
                // Modern Avenir Next look with clean cyan color and thin shadow
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.7:fontsize={}:fontcolor=0x00FFFF:shadowcolor=black@0.6:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, fontsize, start_rel, end_rel
                )
            }
            "vibrant-yellow-box" => {
                // Vibrant black text inside a solid yellow background box (Motivational/TikTok style)
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.72:fontsize={}:fontcolor=black:box=1:boxcolor=0xffff00e0:boxborderw={}:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, fontsize, padding, start_rel, end_rel
                )
            }
            "vibrant-green" => {
                // High-energy neon green text with outline & drop shadow (Hormozi style)
                let borderw = ((fontsize as f64) * 0.08).clamp(1.5, 6.0).round() as i64;
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.7:fontsize={}:fontcolor=0x39FF14:borderw={}:bordercolor=black:shadowcolor=black@0.6:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, fontsize, borderw, start_rel, end_rel
                )
            }
            "vibrant-red" => {
                // Dramatic red text with outline & drop shadow (Gaming/Drama style)
                let borderw = ((fontsize as f64) * 0.08).clamp(1.5, 6.0).round() as i64;
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.7:fontsize={}:fontcolor=0xFF3B30:borderw={}:bordercolor=black:shadowcolor=black@0.6:shadowx=2:shadowy=2:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, fontsize, borderw, start_rel, end_rel
                )
            }
            _ => {
                // modern-box (Default): white text with clean box background
                format!(
                    "drawtext={}text='{}':x=(w-text_w)/2:y=h*0.72:fontsize={}:fontcolor=white:box=1:boxcolor=0x000000b0:boxborderw={}:enable='between(t,{:.3},{:.3})'",
                    font_option, clean_text, fontsize, padding, start_rel, end_rel
                )
            }
        };
        drawtext_filters.push(drawtext);
    }

    drawtext_filters.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TranscriptWord;

    #[test]
    fn test_build_drawtext_filters_formatting() {
        let words = vec![
            TranscriptWord {
                text: "Hello".to_string(),
                start: 0.0,
                end: 1.0,
                speaker: None,
            },
            TranscriptWord {
                text: "world".to_string(),
                start: 1.0,
                end: 2.0,
                speaker: None,
            },
        ];

        let result = build_drawtext_filters(&words, 0.0, 5.0, 1080, "classic-outline");
        assert!(!result.is_empty());
        assert!(result.contains("drawtext="));
        assert!(result.contains("text='HELLO WORLD'"));

        if result.contains("fontfile=") {
            assert!(result.contains("fontfile='"));
            assert!(!result.contains("\\:"));
        }
    }
}

#[tauri::command]
async fn save_instagram_credentials(
    state: tauri::State<'_, AppState>,
    account_id: String,
    access_token: String,
) -> Result<(), String> {
    let acc_id = account_id.trim();
    let token = access_token.trim();

    std::env::set_var("INSTAGRAM_ACCOUNT_ID", acc_id);
    std::env::set_var("INSTAGRAM_ACCESS_TOKEN", token);

    let paths = vec![
        state.data_dir.join(".env"),
        PathBuf::from("/Users/subhajitbepari/Desktop/AutoShorts/.env"),
    ];

    for path in paths {
        let mut lines = Vec::new();
        let mut id_found = false;
        let mut token_found = false;

        if path.exists() {
            if let Ok(file_content) = std::fs::read_to_string(&path) {
                for line in file_content.lines() {
                    if line.starts_with("INSTAGRAM_ACCOUNT_ID=") {
                        lines.push(format!("INSTAGRAM_ACCOUNT_ID={acc_id}"));
                        id_found = true;
                    } else if line.starts_with("INSTAGRAM_ACCESS_TOKEN=") {
                        lines.push(format!("INSTAGRAM_ACCESS_TOKEN={token}"));
                        token_found = true;
                    } else {
                        lines.push(line.to_string());
                    }
                }
            }
        }

        if !id_found {
            lines.push(format!("INSTAGRAM_ACCOUNT_ID={acc_id}"));
        }
        if !token_found {
            lines.push(format!("INSTAGRAM_ACCESS_TOKEN={token}"));
        }

        let _ = std::fs::write(&path, lines.join("\n") + "\n");
    }

    Ok(())
}

#[tauri::command]
async fn test_instagram_connection(
    provider: String,
    account_id: Option<String>,
    access_token: Option<String>,
    webhook_url: Option<String>,
) -> Result<String, String> {
    instagram::test_connection(
        &provider,
        account_id.as_deref(),
        access_token.as_deref(),
        webhook_url.as_deref(),
    )
    .await
    .map_err(to_command_error)
}

#[tauri::command]
async fn publish_candidate_to_instagram(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    caption_override: Option<String>,
    provider: Option<String>,
    account_id: Option<String>,
    access_token: Option<String>,
    webhook_url: Option<String>,
) -> Result<models::InstagramPost, String> {
    let db = state.db.clone();
    let (candidate, project) = db
        .get_candidate_with_project(&candidate_id)
        .map_err(to_command_error)?;

    let clip = db
        .list_clips_for_project(&project.id)
        .map_err(to_command_error)?
        .into_iter()
        .find(|c| c.candidate_id == candidate_id);

    let output_path = match &clip {
        Some(c) if c.status == "done" && c.output_path.is_some() => {
            c.output_path.clone().unwrap()
        }
        _ => {
            // Automatically render clip with 9:16 vertical blur and subtitles
            render_flat_clip_for_candidate(
                state.clone(),
                candidate_id.clone(),
                Some("vertical_crop".to_string()),
                None,
            )
            .await
            .map_err(|e| format!("Failed to automatically render clip for Instagram: {e}"))?
        }
    };

    let active_provider = provider
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| "graph_api".to_string());

    let caption = if let Some(cap) = caption_override.filter(|c| !c.trim().is_empty()) {
        cap
    } else {
        let mut transcript_text = candidate.hook.clone();
        if let Ok(Some(transcript_record)) = db.latest_transcript(&project.id) {
            if let Ok(normalized) = serde_json::from_str::<NormalizedTranscript>(&transcript_record.raw_json) {
                let words: Vec<&str> = normalized
                    .words
                    .iter()
                    .filter(|w| w.start >= candidate.start_sec && w.end <= candidate.end_sec)
                    .map(|w| w.text.as_str())
                    .collect();
                if !words.is_empty() {
                    transcript_text = words.join(" ");
                }
            }
        }
        let kit = llm::generate_social_kit(&candidate.id, &candidate.hook, &transcript_text).await;
        let hashtags = kit.hashtags.join(" ");
        format!(
            "{}\n\n{}\n\n{}\n\n{}",
            candidate.hook, kit.description, kit.call_to_action, hashtags
        )
    };

    let _ = db.upsert_instagram_post(
        &candidate_id,
        clip.as_ref().map(|c| c.id.as_str()),
        "publishing",
        Some(&caption),
        None,
        None,
    );

    let post_result = match active_provider.as_str() {
        "webhook" => {
            let url = webhook_url
                .or_else(|| std::env::var("INSTAGRAM_WEBHOOK_URL").ok())
                .ok_or_else(|| "Instagram Webhook URL is required".to_string())?;

            instagram::publish_reel_webhook(
                &url,
                &candidate_id,
                &output_path,
                &caption,
                candidate.score,
                &candidate.hook,
            )
            .await
        }
        _ => {
            let ig_id = account_id
                .or_else(|| std::env::var("INSTAGRAM_ACCOUNT_ID").ok())
                .ok_or_else(|| "Instagram Account ID is required".to_string())?;
            let token = access_token
                .or_else(|| std::env::var("INSTAGRAM_ACCESS_TOKEN").ok())
                .ok_or_else(|| "Instagram Meta Access Token is required".to_string())?;

            instagram::publish_reel_graph_api(&ig_id, &token, &output_path, &caption).await
        }
    };

    match post_result {
        Ok(post_url) => {
            let updated = db
                .upsert_instagram_post(
                    &candidate_id,
                    clip.as_ref().map(|c| c.id.as_str()),
                    "published",
                    Some(&caption),
                    Some(&post_url),
                    None,
                )
                .map_err(to_command_error)?;
            Ok(updated)
        }
        Err(err) => {
            let err_msg = err.to_string();
            let _ = db.upsert_instagram_post(
                &candidate_id,
                clip.as_ref().map(|c| c.id.as_str()),
                "failed",
                Some(&caption),
                None,
                Some(&err_msg),
            );
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn auto_publish_eligible_candidates(
    state: tauri::State<'_, AppState>,
    project_id: String,
    min_score: Option<f64>,
    provider: Option<String>,
    account_id: Option<String>,
    access_token: Option<String>,
    webhook_url: Option<String>,
) -> Result<Vec<models::InstagramPost>, String> {
    let db = state.db.clone();
    let threshold = min_score.unwrap_or(0.90);
    let candidates = db.list_candidates(&project_id).map_err(to_command_error)?;
    let mut results = Vec::new();

    for c in candidates {
        let normalized_score = if c.score > 1.0 { c.score / 100.0 } else { c.score };
        if normalized_score >= threshold || c.score >= (threshold * 100.0) {
            let clip = db
                .list_clips_for_project(&project_id)
                .map_err(to_command_error)?
                .into_iter()
                .find(|cl| cl.candidate_id == c.id);

            let is_rendered = clip.as_ref().map(|cl| cl.status == "done" && cl.output_path.is_some()).unwrap_or(false);
            if !is_rendered {
                let _ = render_flat_clip_for_candidate(state.clone(), c.id.clone(), Some("vertical_crop".to_string()), None).await;
            }

            match publish_candidate_to_instagram(
                state.clone(),
                c.id.clone(),
                None,
                provider.clone(),
                account_id.clone(),
                access_token.clone(),
                webhook_url.clone(),
            )
            .await
            {
                Ok(post) => results.push(post),
                Err(err) => {
                    eprintln!("Auto-publish candidate {} failed: {}", c.id, err);
                }
            }
        }
    }
    Ok(results)
}
