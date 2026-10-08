use std::time::Duration;
use tauri::Emitter;

use crate::credentials;
use crate::media;
use crate::models::EnvironmentStatus;
use crate::transcription;
use crate::AppState;

#[derive(Clone, serde::Serialize)]
pub struct PullProgressPayload {
    pub status: String,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub percentage: Option<f64>,
}

pub async fn get_environment_status(state: &AppState) -> Result<EnvironmentStatus, String> {
    // 1. Ensure .env is loaded from data_dir and workspace roots
    let env_file = state.data_dir.join(".env");
    if env_file.exists() {
        let _ = dotenvy::from_path(&env_file);
    }
    let _ = dotenvy::dotenv();

    let llm_provider = std::env::var("LLM_PROVIDER")
        .unwrap_or_else(|_| "deepseek".to_string())
        .to_lowercase();

    let has_local_whisper_model =
        transcription::whisper_cli_exists() || transcription::whisper_python_exists();

    let has_ollama = reqwest::Client::new()
        .get("http://localhost:11434")
        .timeout(Duration::from_millis(1000))
        .send()
        .await
        .is_ok();

    let platform = std::env::consts::OS.to_string();
    let local_whisper_supported = true;
    let ollama_supported = true;
    let ollama_install_supported = cfg!(target_os = "macos");
    let face_tracking_supported = cfg!(target_os = "macos");
    let multi_speaker_reframing_supported = face_tracking_supported;
    let hardware_encoder_supported = media::supports_videotoolbox();

    let has_nvidia_key = credentials::has(credentials::NVIDIA).unwrap_or(false);
    let has_nvidia_function_id = credentials::has(credentials::NVIDIA_FUNCTION_ID).unwrap_or(false);

    let (active_speaker_provider, active_speaker_status) =
        if has_nvidia_key && has_nvidia_function_id {
            (
                "NVIDIA".to_string(),
                Some(
                    "Ready (NVIDIA ASD NIM active speaker detection with Apple Vision fusion)"
                        .to_string(),
                ),
            )
        } else if has_nvidia_key {
            (
                "Local fallback".to_string(),
                Some(
                    "NVIDIA API Key set, but NVCF Function ID missing. Using Local fallback."
                        .to_string(),
                ),
            )
        } else {
            (
            "Local fallback".to_string(),
            Some(
                "NVIDIA API key not set. Using Local fallback (Apple Vision + Diarization fusion)."
                    .to_string(),
            ),
        )
        };

    let has_youtube_config = credentials::has(credentials::YOUTUBE_CLIENT_ID).unwrap_or(false)
        && credentials::has(credentials::YOUTUBE_CLIENT_SECRET).unwrap_or(false)
        && credentials::has(credentials::YOUTUBE_REFRESH_TOKEN).unwrap_or(false);

    Ok(EnvironmentStatus {
        data_dir: state.data_dir.to_string_lossy().to_string(),
        has_ffmpeg: media::command_exists("ffmpeg"),
        has_ffprobe: media::command_exists("ffprobe"),
        has_deepgram_key: credentials::has(credentials::DEEPGRAM).unwrap_or(false),
        has_anthropic_key: credentials::has(credentials::ANTHROPIC).unwrap_or(false),
        has_deepseek_key: credentials::has(credentials::DEEPSEEK).unwrap_or(false),
        has_gemini_key: credentials::has(credentials::GEMINI).unwrap_or(false),
        has_openai_key: credentials::has(credentials::OPENAI).unwrap_or(false),
        has_openrouter_key: credentials::has(credentials::OPENROUTER).unwrap_or(false),
        has_groq_key: credentials::has(credentials::GROQ).unwrap_or(false),
        has_nvidia_key,
        has_nvidia_function_id,
        active_speaker_provider,
        active_speaker_status,
        has_instagram_token: credentials::has(credentials::INSTAGRAM).unwrap_or(false),
        has_youtube_config,
        llm_provider,
        has_local_whisper_model,
        has_ollama,
        has_ytdlp: media::command_exists("yt-dlp"),
        has_hardware_accel: hardware_encoder_supported,
        instagram_account_id: std::env::var("INSTAGRAM_ACCOUNT_ID").ok(),
        platform,
        local_whisper_supported,
        ollama_supported,
        ollama_install_supported,
        face_tracking_supported,
        multi_speaker_reframing_supported,
        hardware_encoder_supported,
    })
}

pub async fn pull_ollama_model(app: &tauri::AppHandle, model_name: &str) -> Result<(), String> {
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

                let mut status = val
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Downloading...")
                    .to_string();

                if status.starts_with("downloading ") {
                    if let (Some(c), Some(t)) = (completed, total) {
                        let c_mb = c as f64 / 1024.0 / 1024.0;
                        let t_mb = t as f64 / 1024.0 / 1024.0;
                        if t_mb > 100.0 {
                            status =
                                format!("Downloading weights: {:.1} MB / {:.1} MB", c_mb, t_mb);
                        } else {
                            status = format!(
                                "Downloading model components: {:.1} MB / {:.1} MB",
                                c_mb, t_mb
                            );
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

pub async fn install_ollama(app: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        let msg = format!(
            "Automatic in-app Ollama installation is not supported on {}. Please install Ollama from https://ollama.com and start the service.",
            std::env::consts::OS
        );
        let _ = app.emit("ollama-install-status", &msg);
        return Err(msg);
    }

    #[cfg(target_os = "macos")]
    {
        let _ = app.emit(
            "ollama-install-status",
            "Checking if Ollama is already installed...",
        );
        let launch = std::process::Command::new("open")
            .args(["-a", "Ollama"])
            .output();

        if let Ok(out) = launch {
            if out.status.success() {
                let _ = app.emit("ollama-install-status", "Ollama is installed. Launching...");
                for _ in 0..12 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if reqwest::Client::new()
                        .get("http://localhost:11434")
                        .send()
                        .await
                        .is_ok()
                    {
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
            let _ = app.emit(
                "ollama-install-status",
                "Installing Ollama via Homebrew Cask...",
            );

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
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        if reqwest::Client::new()
                            .get("http://localhost:11434")
                            .send()
                            .await
                            .is_ok()
                        {
                            let _ =
                                app.emit("ollama-install-status", "Ollama started successfully!");
                            return Ok(());
                        }
                    }
                }
            }
        }

        let _ = app.emit(
            "ollama-install-status",
            "Downloading Ollama zip from official source...",
        );
        let temp_dir = std::env::temp_dir();
        let zip_path = temp_dir.join("Ollama-darwin.zip");

        let response = reqwest::get("https://ollama.com/download/Ollama-darwin.zip")
            .await
            .map_err(|e| format!("Failed to download Ollama: {e}"))?;

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read Ollama bytes: {e}"))?;
        std::fs::write(&zip_path, bytes).map_err(|e| format!("Failed to save Ollama zip: {e}"))?;

        let _ = app.emit("ollama-install-status", "Unzipping Ollama package...");
        let unzip_output = std::process::Command::new("unzip")
            .args([
                "-o",
                zip_path.to_string_lossy().as_ref(),
                "-d",
                temp_dir.to_string_lossy().as_ref(),
            ])
            .output()
            .map_err(|e| format!("Failed to unzip Ollama: {e}"))?;

        if !unzip_output.status.success() {
            return Err(format!(
                "Failed to unzip: {}",
                String::from_utf8_lossy(&unzip_output.stderr)
            ));
        }

        let _ = app.emit(
            "ollama-install-status",
            "Installing to Applications folder...",
        );
        let app_src = temp_dir.join("Ollama.app");

        let mv_output = std::process::Command::new("mv")
            .args([app_src.to_string_lossy().as_ref(), "/Applications/"])
            .output()
            .map_err(|e| format!("Failed to move Ollama to Applications: {e}"))?;

        if !mv_output.status.success() {
            let user_apps = dirs::home_dir()
                .ok_or_else(|| "Could not find home directory".to_string())?
                .join("Applications");
            std::fs::create_dir_all(&user_apps)
                .map_err(|e| format!("Failed to create ~/Applications: {e}"))?;

            let mv_user_output = std::process::Command::new("mv")
                .args([
                    &app_src.to_string_lossy().to_string(),
                    &user_apps.to_string_lossy().to_string(),
                ])
                .output()
                .map_err(|e| format!("Failed to move Ollama to ~/Applications: {e}"))?;

            if !mv_user_output.status.success() {
                return Err(format!(
                    "Failed to install Ollama to Applications folder: {}",
                    String::from_utf8_lossy(&mv_user_output.stderr)
                ));
            }
        }

        let _ = app.emit("ollama-install-status", "Starting Ollama...");
        let launch = std::process::Command::new("open")
            .args(["-a", "Ollama"])
            .output();

        if launch.is_ok() {
            for _ in 0..12 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if reqwest::Client::new()
                    .get("http://localhost:11434")
                    .send()
                    .await
                    .is_ok()
                {
                    let _ = app.emit("ollama-install-status", "Ollama started successfully!");
                    return Ok(());
                }
            }
        }

        Err("Ollama installed but could not be automatically started. Please open Ollama from your Applications folder.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_status_capabilities_serde() {
        let status = EnvironmentStatus {
            data_dir: "/tmp/data".to_string(),
            has_ffmpeg: true,
            has_ffprobe: true,
            has_deepgram_key: false,
            has_anthropic_key: false,
            has_deepseek_key: false,
            has_gemini_key: false,
            has_openai_key: false,
            has_openrouter_key: false,
            has_groq_key: false,
            has_nvidia_key: false,
            has_nvidia_function_id: false,
            active_speaker_provider: "Local fallback".to_string(),
            active_speaker_status: Some("Active (Local fallback)".to_string()),
            has_instagram_token: false,
            has_youtube_config: false,
            llm_provider: "deepseek".to_string(),
            has_local_whisper_model: false,
            has_ollama: false,
            has_ytdlp: true,
            has_hardware_accel: true,
            instagram_account_id: None,
            platform: "macos".to_string(),
            local_whisper_supported: true,
            ollama_supported: true,
            ollama_install_supported: true,
            face_tracking_supported: true,
            multi_speaker_reframing_supported: true,
            hardware_encoder_supported: true,
        };

        let json = serde_json::to_string(&status).expect("must serialize");
        assert!(json.contains("\"platform\":\"macos\""));
        assert!(json.contains("\"ollamaInstallSupported\":true"));
        assert!(json.contains("\"faceTrackingSupported\":true"));
        assert!(json.contains("\"multiSpeakerReframingSupported\":true"));
        assert!(json.contains("\"activeSpeakerProvider\":\"Local fallback\""));

        let deserialized: EnvironmentStatus =
            serde_json::from_str(&json).expect("must deserialize");
        assert_eq!(deserialized.platform, "macos");
        assert_eq!(deserialized.active_speaker_provider, "Local fallback");
        assert!(deserialized.local_whisper_supported);
        assert!(deserialized.ollama_supported);
        assert!(deserialized.ollama_install_supported);
        assert!(deserialized.face_tracking_supported);
        assert!(deserialized.multi_speaker_reframing_supported);
        assert!(deserialized.hardware_encoder_supported);
    }
}
