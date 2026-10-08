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
fn default_local_provider() -> String {
    "Local fallback".to_string()
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
    #[serde(default)]
    pub has_nvidia_function_id: bool,
    #[serde(default = "default_local_provider")]
    pub active_speaker_provider: String,
    #[serde(default)]
    pub active_speaker_status: Option<String>,
    pub has_instagram_token: bool,
    #[serde(default)]
    pub has_youtube_config: bool,
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
    #[serde(default = "default_false", alias = "dynamic_podcast_supported")]
    pub multi_speaker_reframing_supported: bool,
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
    #[serde(default)]
    pub social_kit: Option<SocialKit>,
    #[serde(default)]
    pub quality_score: Option<ClipQualityScore>,
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
pub struct YouTubePost {
    pub id: String,
    pub candidate_id: String,
    pub clip_id: Option<String>,
    pub status: String,
    pub title: Option<String>,
    pub video_id: Option<String>,
    pub video_url: Option<String>,
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
    #[serde(default)]
    pub youtube_posts: Vec<YouTubePost>,
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
    #[serde(default)]
    pub quality_score: Option<ClipQualityScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClipQualityScore {
    pub hook: Option<f64>,
    pub coherence: Option<f64>,
    pub context_independence: Option<f64>,
    pub payoff: Option<f64>,
    pub speech_quality: Option<f64>,
    pub visual_quality: Option<f64>,
    pub boundary_quality: Option<f64>,
    pub redundancy_penalty: f64,
    pub risk_penalty: f64,
    pub total: f64,
    pub version: u32,
}

impl ClipQualityScore {
    pub fn compute(
        hook: f64,
        coherence: f64,
        context_independence: f64,
        payoff: f64,
        speech_quality: f64,
        visual_quality: f64,
        boundary_quality: f64,
        redundancy_penalty: f64,
        risk_penalty: f64,
    ) -> Self {
        let positive = 0.20 * hook.clamp(0.0, 1.0)
            + 0.18 * coherence.clamp(0.0, 1.0)
            + 0.15 * context_independence.clamp(0.0, 1.0)
            + 0.15 * payoff.clamp(0.0, 1.0)
            + 0.12 * speech_quality.clamp(0.0, 1.0)
            + 0.10 * visual_quality.clamp(0.0, 1.0)
            + 0.10 * boundary_quality.clamp(0.0, 1.0);
        let penalty = redundancy_penalty.clamp(0.0, 0.5) + risk_penalty.clamp(0.0, 0.5);
        let total = ((positive - penalty) * 100.0).clamp(0.0, 100.0);
        Self {
            hook: Some(hook),
            coherence: Some(coherence),
            context_independence: Some(context_independence),
            payoff: Some(payoff),
            speech_quality: Some(speech_quality),
            visual_quality: Some(visual_quality),
            boundary_quality: Some(boundary_quality),
            redundancy_penalty,
            risk_penalty,
            total: (total * 10.0).round() / 10.0,
            version: 1,
        }
    }

    pub fn apply_redundancy_penalty(&mut self, penalty: f64) {
        self.redundancy_penalty = (self.redundancy_penalty + penalty).clamp(0.0, 0.5);
        let hook_val = self.hook.unwrap_or(0.7).clamp(0.0, 1.0);
        let coherence_val = self.coherence.unwrap_or(0.7).clamp(0.0, 1.0);
        let context_val = self.context_independence.unwrap_or(0.7).clamp(0.0, 1.0);
        let payoff_val = self.payoff.unwrap_or(0.7).clamp(0.0, 1.0);
        let speech_val = self.speech_quality.unwrap_or(0.7).clamp(0.0, 1.0);
        let visual_val = self.visual_quality.unwrap_or(0.7).clamp(0.0, 1.0);
        let boundary_val = self.boundary_quality.unwrap_or(0.7).clamp(0.0, 1.0);

        let positive = 0.20 * hook_val
            + 0.18 * coherence_val
            + 0.15 * context_val
            + 0.15 * payoff_val
            + 0.12 * speech_val
            + 0.10 * visual_val
            + 0.10 * boundary_val;
        let penalty_val =
            self.redundancy_penalty.clamp(0.0, 0.5) + self.risk_penalty.clamp(0.0, 0.5);
        self.total = (((positive - penalty_val) * 100.0).clamp(0.0, 100.0) * 10.0).round() / 10.0;
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateFeedback {
    pub id: String,
    pub candidate_id: String,
    pub project_id: String,
    pub action: String,
    pub rating: Option<i64>,
    pub details_json: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_quality_score_computation_and_penalties() {
        let perfect = ClipQualityScore::compute(1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0);
        assert_eq!(perfect.total, 100.0);
        assert_eq!(perfect.version, 1);

        let zero = ClipQualityScore::compute(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(zero.total, 0.0);

        let mut scored = ClipQualityScore::compute(0.9, 0.85, 0.8, 0.9, 0.75, 0.8, 0.9, 0.0, 0.0);
        let initial_total = scored.total;
        assert!(initial_total > 80.0 && initial_total < 90.0);

        // Apply redundancy penalty
        scored.apply_redundancy_penalty(0.15);
        assert!(scored.total < initial_total);
        assert_eq!(scored.redundancy_penalty, 0.15);

        // Serde roundtrip preserves camelCase
        let json = serde_json::to_string(&scored).expect("serialize");
        assert!(json.contains("contextIndependence"));
        assert!(json.contains("speechQuality"));
        let parsed: ClipQualityScore = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, scored);
    }
}
