use crate::jobs::JobInfo;
use crate::models::{
    Candidate, EnvironmentStatus, InstagramPost, MediaProbe, Project, ProjectDetail, SocialKit,
    Transcript, YouTubePost,
};
use crate::services::candidate_service;
use crate::services::credential_service;
use crate::services::environment_service;
use crate::services::instagram_service;
use crate::services::project_service::{self, CopyrightCheckResult, DefaultFolders};
use crate::services::render_service;
use crate::services::transcription_service;
use crate::services::youtube_service;
use crate::AppState;

#[tauri::command]
pub async fn environment_status(
    state: tauri::State<'_, AppState>,
) -> Result<EnvironmentStatus, String> {
    environment_service::get_environment_status(&state).await
}

#[tauri::command]
pub async fn pull_ollama_model(app: tauri::AppHandle, model_name: String) -> Result<(), String> {
    environment_service::pull_ollama_model(&app, &model_name).await
}

#[tauri::command]
pub async fn install_ollama(app: tauri::AppHandle) -> Result<(), String> {
    environment_service::install_ollama(&app).await
}

#[tauri::command]
pub fn create_project_from_path(
    state: tauri::State<'_, AppState>,
    path: String,
    transcription_mode: String,
    caption_style: String,
) -> Result<Project, String> {
    project_service::create_project_from_path(&state, &path, &transcription_mode, &caption_style)
}

#[tauri::command]
pub fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<Project>, String> {
    project_service::list_projects(&state)
}

#[tauri::command]
pub fn get_project_detail(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<ProjectDetail, String> {
    project_service::get_project_detail(&state, &project_id)
}

#[tauri::command]
pub fn probe_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<MediaProbe, String> {
    project_service::probe_project(&state, &project_id)
}

#[tauri::command]
pub fn extract_project_audio(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<String, String> {
    transcription_service::extract_project_audio(&state, &project_id)
}

#[tauri::command]
pub async fn transcribe_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
    provider: String,
) -> Result<Transcript, String> {
    transcription_service::transcribe_project(&state, &project_id, &provider).await
}

#[tauri::command]
pub fn save_demo_transcript(
    state: tauri::State<'_, AppState>,
    project_id: String,
) -> Result<Transcript, String> {
    transcription_service::save_demo_transcript(&state, &project_id)
}

#[tauri::command]
pub async fn generate_candidates(
    state: tauri::State<'_, AppState>,
    project_id: String,
    provider: Option<String>,
    model_name: Option<String>,
    allow_demo: bool,
) -> Result<Vec<Candidate>, String> {
    candidate_service::generate_candidates(&state, &project_id, provider, model_name, allow_demo).await
}

#[tauri::command]
pub fn set_selected_clip_count(
    state: tauri::State<'_, AppState>,
    project_id: String,
    count: usize,
) -> Result<Vec<Candidate>, String> {
    candidate_service::set_selected_clip_count(&state, &project_id, count)
}

#[tauri::command]
pub async fn update_candidate_timing(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    start_sec: f64,
    end_sec: f64,
) -> Result<Candidate, String> {
    candidate_service::update_candidate_timing(&state, &candidate_id, start_sec, end_sec).await
}


#[tauri::command]
pub async fn render_flat_clip_for_candidate(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    reframe_mode: Option<String>,
    output_dir: Option<String>,
    remove_silence: Option<bool>,
    punch_zoom: Option<bool>,
    studio_audio: Option<bool>,
    export_preset: Option<String>,
) -> Result<String, String> {
    render_service::render_flat_clip_for_candidate(
        app,
        state.inner().clone(),
        candidate_id,
        reframe_mode,
        output_dir,
        remove_silence,
        punch_zoom,
        studio_audio,
        export_preset,
    )
    .await
}

#[tauri::command]
pub fn delete_project(state: tauri::State<'_, AppState>, project_id: String) -> Result<(), String> {
    project_service::delete_project(&state, &project_id)
}

#[tauri::command]
pub fn rename_project(
    state: tauri::State<'_, AppState>,
    project_id: String,
    name: String,
) -> Result<(), String> {
    project_service::rename_project(&state, &project_id, &name)
}

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    project_service::open_folder(&path)
}

#[tauri::command]
pub fn get_default_folders() -> Result<DefaultFolders, String> {
    project_service::get_default_folders()
}

#[tauri::command]
pub async fn check_youtube_copyright(url: String) -> Result<CopyrightCheckResult, String> {
    project_service::check_youtube_copyright(&url).await
}

#[tauri::command]
pub async fn download_youtube_video(
    url: String,
    output_dir: Option<String>,
    user_acknowledged: Option<bool>,
) -> Result<String, String> {
    project_service::download_youtube_video(&url, output_dir.as_deref(), user_acknowledged).await
}

#[tauri::command]
pub async fn generate_social_kit_for_candidate(
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    provider: Option<String>,
    model_name: Option<String>,
) -> Result<SocialKit, String> {
    candidate_service::generate_social_kit_for_candidate(
        &state,
        &candidate_id,
        provider,
        model_name,
    )
    .await
}

#[tauri::command]
pub async fn test_instagram_connection(
    provider: String,
    account_id: Option<String>,
    access_token: Option<String>,
    webhook_url: Option<String>,
) -> Result<String, String> {
    instagram_service::test_instagram_connection(
        &provider,
        account_id,
        access_token,
        webhook_url,
    )
    .await
}

#[tauri::command]
pub async fn publish_candidate_to_instagram(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    caption_override: Option<String>,
    provider: Option<String>,
    account_id: Option<String>,
    access_token: Option<String>,
    webhook_url: Option<String>,
) -> Result<InstagramPost, String> {
    instagram_service::publish_candidate_to_instagram(
        app,
        state.inner().clone(),
        candidate_id,
        caption_override,
        provider,
        account_id,
        access_token,
        webhook_url,
    )
    .await
}

#[tauri::command]
pub async fn save_instagram_credentials(
    _state: tauri::State<'_, AppState>,
    account_id: String,
    access_token: String,
) -> Result<(), String> {
    credential_service::save_instagram_credentials(&account_id, &access_token)
}

#[tauri::command]
pub async fn test_youtube_connection(
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
) -> Result<String, String> {
    youtube_service::test_youtube_connection(client_id, client_secret, refresh_token).await
}

#[tauri::command]
pub async fn publish_candidate_to_youtube(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    candidate_id: String,
    title_override: Option<String>,
    description_override: Option<String>,
    privacy_status: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
) -> Result<YouTubePost, String> {
    youtube_service::publish_candidate_to_youtube(
        app,
        state.inner().clone(),
        candidate_id,
        title_override,
        description_override,
        privacy_status,
        client_id,
        client_secret,
        refresh_token,
    )
    .await
}

#[tauri::command]
pub async fn save_youtube_credentials(
    _state: tauri::State<'_, AppState>,
    client_id: String,
    client_secret: String,
    refresh_token: String,
) -> Result<(), String> {
    credential_service::save_youtube_credentials(&client_id, &client_secret, &refresh_token)
}

#[tauri::command]
pub async fn save_credential(name: String, value: String) -> Result<(), String> {
    credential_service::save_credential(&name, &value)
}

#[tauri::command]
pub async fn delete_credential(name: String) -> Result<(), String> {
    credential_service::delete_credential(&name)
}

#[tauri::command]
pub async fn credential_status(name: String) -> Result<bool, String> {
    credential_service::credential_status(&name)
}

macro_rules! credential_status_command {
    ($name:ident) => {
        #[tauri::command]
        pub async fn $name() -> Result<bool, String> {
            credential_service::$name()
        }
    };
}

credential_status_command!(is_deepgram_configured);
credential_status_command!(is_gemini_configured);
credential_status_command!(is_openai_configured);
credential_status_command!(is_anthropic_configured);
credential_status_command!(is_deepseek_configured);
credential_status_command!(is_groq_configured);
credential_status_command!(is_openrouter_configured);
credential_status_command!(is_nvidia_configured);
credential_status_command!(is_instagram_configured);
credential_status_command!(is_youtube_configured);

#[tauri::command]
pub async fn clear_all_storage(state: tauri::State<'_, AppState>) -> Result<String, String> {
    project_service::clear_all_storage(&state)
}

#[tauri::command]
pub async fn cancel_job(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    job_id: String,
) -> Result<(), String> {
    render_service::cancel_job(&app, &state, &job_id)
}

#[tauri::command]
pub async fn get_active_jobs(state: tauri::State<'_, AppState>) -> Result<Vec<JobInfo>, String> {
    render_service::get_active_jobs(&state)
}
