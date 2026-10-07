use serde::{Deserialize, Serialize};

fn default_platform() -> String {
    std::env::consts::OS.to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStatus {
    pub data_dir: String,
    pub has_ffmpeg: bool,
    pub has_ffprobe: bool,
    pub has_deepgram_key: bool,
    pub has_anthropic_key: bool,
    pub has_deepseek_key: bool,
    pub has_gemini_key: bool,
    pub has_openai_key: bool,
    pub has_openrouter_key: bool,
    pub has_groq_key: bool,
    pub has_nvidia_key: bool,
    pub has_instagram_token: bool,
    pub llm_provider: String,
    pub has_local_whisper_model: bool,
    pub has_ollama: bool,
    pub has_ytdlp: bool,
    pub has_hardware_accel: bool,
    #[serde(default)]
    pub instagram_account_id: Option<String>,
    #[serde(default = "default_platform")]
    pub platform: String,
    #[serde(default = "default_true")]
    pub local_whisper_supported: bool,
    #[serde(default = "default_true")]
    pub ollama_supported: bool,
    #[serde(default = "default_false")]
    pub ollama_install_supported: bool,
    #[serde(default = "default_false")]
    pub face_tracking_supported: bool,
    #[serde(default = "default_false")]
    pub dynamic_podcast_supported: bool,
    #[serde(default = "default_false")]
    pub hardware_encoder_supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbe {
    pub duration_sec: Option<f64>,
    pub has_video: bool,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: Option<String>,
    pub source_path: String,
    pub source_duration: Option<f64>,
    pub status: String,
    pub transcription_mode: String,
    pub caption_style: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub id: String,
    pub project_id: String,
    pub engine: String,
    pub raw_json: String,
    pub language: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub project_id: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub score: f64,
    pub hook: String,
    pub rationale: String,
    pub rank: i64,
    pub selected: bool,
    pub layout_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub candidate_id: String,
    pub status: String,
    pub output_path: Option<String>,
    pub face_track_json: Option<String>,
    pub caption_ass_path: Option<String>,
    pub render_log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipCopy {
    pub id: String,
    pub clip_id: String,
    pub platform: String,
    pub hook_text: Option<String>,
    pub caption_text: Option<String>,
    pub hashtags: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstagramPost {
    pub id: String,
    pub candidate_id: String,
    pub clip_id: Option<String>,
    pub status: String,
    pub caption: Option<String>,
    pub post_url: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetail {
    pub project: Project,
    pub transcript: Option<Transcript>,
    pub candidates: Vec<Candidate>,
    pub clips: Vec<Clip>,
    pub copy: Vec<ClipCopy>,
    #[serde(default)]
    pub instagram_posts: Vec<InstagramPost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedTranscript {
    pub language: String,
    pub duration: f64,
    pub speakers: Vec<String>,
    pub words: Vec<TranscriptWord>,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub start: f64,
    pub end: f64,
    pub speaker: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateDraft {
    pub start: f64,
    pub end: f64,
    pub score: f64,
    pub hook: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialKit {
    pub candidate_id: String,
    pub titles: Vec<String>,
    pub description: String,
    pub hashtags: Vec<String>,
    pub call_to_action: String,
}
