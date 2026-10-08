pub mod analysis_cache;
pub mod commands;
pub mod credentials;
pub mod db;
pub mod http_client;
pub mod instagram;
pub mod jobs;
pub mod llm;
pub mod media;
pub mod models;
pub mod pro_editor;
pub mod services;
pub mod transcription;
pub mod youtube_uploader;

use std::path::PathBuf;
use anyhow::Context;
use tauri::Manager;

use db::Database;

pub use services::project_service::{
    canonicalize_youtube_url, extract_youtube_video_id, CopyrightCheckResult, DefaultFolders,
};
pub use services::render_service::build_drawtext_filters;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub data_dir: PathBuf,
    pub jobs: jobs::JobManager,
}

pub fn run() {
    let _ = dotenvy::dotenv();

    let current_path = std::env::var("PATH").unwrap_or_default();
    let extended_path = format!(
        "/opt/homebrew/bin:/opt/homebrew/opt/ffmpeg-full/bin:/usr/local/bin:/usr/bin:/bin:{}",
        current_path
    );
    std::env::set_var("PATH", extended_path);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .context("resolving app data directory")?;
            std::fs::create_dir_all(&data_dir).context("creating app data directory")?;
            std::fs::create_dir_all(data_dir.join("models"))
                .context("creating models directory")?;
            let candidates = [
                data_dir.join(".env"),
                PathBuf::from(".env"),
                PathBuf::from("../.env"),
            ];
            for cand in &candidates {
                if cand.exists() {
                    let _ = dotenvy::from_path(cand);
                }
            }
            let _ = dotenvy::dotenv();
            credentials::init_all_credentials();
            let db_path = if data_dir.join("clipon.sqlite").exists() {
                data_dir.join("clipon.sqlite")
            } else if data_dir.join("autoshorts.sqlite").exists() {
                let _ = std::fs::copy(
                    data_dir.join("autoshorts.sqlite"),
                    data_dir.join("clipon.sqlite"),
                );
                data_dir.join("clipon.sqlite")
            } else {
                data_dir.join("clipon.sqlite")
            };
            let db = Database::open(&db_path)?;
            media::cleanup_stale_temp_dirs();
            analysis_cache::AnalysisCache::init_global(&data_dir);
            app.manage(AppState {
                db,
                data_dir,
                jobs: jobs::JobManager::global().clone(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::environment_status,
            commands::pull_ollama_model,
            commands::install_ollama,
            commands::create_project_from_path,
            commands::list_projects,
            commands::get_project_detail,
            commands::probe_project,
            commands::extract_project_audio,
            commands::transcribe_project,
            commands::save_demo_transcript,
            commands::generate_candidates,
            commands::set_selected_clip_count,
            commands::render_flat_clip_for_candidate,
            commands::delete_project,
            commands::rename_project,
            commands::check_youtube_copyright,
            commands::download_youtube_video,
            commands::open_folder,
            commands::get_default_folders,
            commands::generate_social_kit_for_candidate,
            commands::test_instagram_connection,
            commands::publish_candidate_to_instagram,
            commands::save_instagram_credentials,
            commands::test_youtube_connection,
            commands::publish_candidate_to_youtube,
            commands::save_youtube_credentials,
            commands::save_credential,
            commands::delete_credential,
            commands::credential_status,
            commands::is_deepgram_configured,
            commands::is_gemini_configured,
            commands::is_openai_configured,
            commands::is_anthropic_configured,
            commands::is_deepseek_configured,
            commands::is_groq_configured,
            commands::is_openrouter_configured,
            commands::is_nvidia_configured,
            commands::is_instagram_configured,
            commands::is_youtube_configured,
            commands::update_candidate_timing,
            commands::clear_all_storage,
            commands::cancel_job,
            commands::get_active_jobs,
            commands::record_candidate_feedback,
            commands::list_candidate_feedback,
            commands::clear_candidate_feedback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ClipOn");
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TranscriptWord;
    use crate::services::project_service::project_output_slug;

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

    #[test]
    fn test_extract_and_canonicalize_youtube_urls() {
        // Standard watch URLs
        assert_eq!(
            canonicalize_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            canonicalize_youtube_url("https://youtube.com/watch?v=dQw4w9WgXcQ&t=42s").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            canonicalize_youtube_url("https://m.youtube.com/watch?v=dQw4w9WgXcQ").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );

        // youtu.be shortlinks
        assert_eq!(
            canonicalize_youtube_url("https://youtu.be/dQw4w9WgXcQ").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            canonicalize_youtube_url("youtu.be/dQw4w9WgXcQ?si=abcdef").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );

        // Shorts and Embeds
        assert_eq!(
            canonicalize_youtube_url("https://www.youtube.com/shorts/dQw4w9WgXcQ").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            canonicalize_youtube_url("https://www.youtube.com/embed/dQw4w9WgXcQ").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );

        // Direct 11-char ID
        assert_eq!(
            canonicalize_youtube_url("dQw4w9WgXcQ").unwrap(),
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        );

        // Security rejections: non-YouTube domains
        assert!(canonicalize_youtube_url("https://vimeo.com/12345678").is_err());
        assert!(canonicalize_youtube_url("https://malicious.org/video").is_err());
        assert!(canonicalize_youtube_url("https://notyoutube.com/watch?v=dQw4w9WgXcQ").is_err());

        // Invalid length or empty
        assert!(canonicalize_youtube_url("").is_err());
        assert!(canonicalize_youtube_url("https://www.youtube.com/watch?v=too_short").is_err());
    }

    #[test]
    fn test_project_output_slug_avoids_collisions() {
        let p1 = models::Project {
            id: "proj1_uuid_12345678".to_string(),
            name: None,
            source_path: "/path/to/interview.mp4".to_string(),
            source_duration: Some(100.0),
            status: "ready".to_string(),
            transcription_mode: "deepgram".to_string(),
            caption_style: None,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        };

        let p2 = models::Project {
            id: "proj2_uuid_87654321".to_string(),
            name: None,
            source_path: "/other/path/to/interview.mp4".to_string(),
            source_duration: Some(100.0),
            status: "ready".to_string(),
            transcription_mode: "deepgram".to_string(),
            caption_style: None,
            created_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
        };

        let slug1 = project_output_slug(&p1);
        let slug2 = project_output_slug(&p2);

        assert_eq!(slug1, "interview-proj1_uu");
        assert_eq!(slug2, "interview-proj2_uu");
        assert_ne!(slug1, slug2, "Projects with same source filename must have distinct output slugs");
    }
}
