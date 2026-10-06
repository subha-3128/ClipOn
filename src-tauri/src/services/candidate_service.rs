use crate::credentials;
use crate::llm;
use crate::models::{Candidate, NormalizedTranscript, SocialKit};
use crate::pro_editor;
use crate::AppState;

pub async fn generate_candidates(
    state: &AppState,
    project_id: &str,
    provider: Option<String>,
    model_name: Option<String>,
    _allow_demo: bool,
) -> Result<Vec<Candidate>, String> {
    let db = state.db.clone();
    let transcript = db
        .latest_transcript(project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Transcribe the project before detecting moments.".to_string())?;
    let normalized: NormalizedTranscript =
        serde_json::from_str(&transcript.raw_json).map_err(|e| e.to_string())?;

    let active_provider = provider
        .or_else(|| std::env::var("LLM_PROVIDER").ok())
        .unwrap_or_else(|| "deepseek".to_string())
        .to_lowercase();

    let drafts = match active_provider.as_str() {
        "claude" => {
            let key = credentials::get(credentials::ANTHROPIC)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    "Configure an Anthropic API key in Settings to generate candidates.".to_string()
                })?;
            llm::detect_candidates_with_claude(&normalized, &key)
                .await
                .map_err(|e| e.to_string())?
        }
        "local" | "ollama" => {
            let model = model_name
                .filter(|m| !m.trim().is_empty())
                .or_else(|| std::env::var("OLLAMA_MODEL").ok())
                .unwrap_or_else(|| "llama3.2".to_string());
            llm::detect_candidates_with_local_llm(&normalized, &model)
                .await
                .map_err(|e| e.to_string())?
        }
        "gemini" => {
            let key = credentials::get(credentials::GEMINI)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    "Configure a Gemini API key in Settings to generate candidates.".to_string()
                })?;
            llm::detect_candidates_with_gemini(&normalized, &key)
                .await
                .map_err(|e| e.to_string())?
        }
        "openai" => {
            let key = credentials::get(credentials::OPENAI)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    "Configure an OpenAI API key in Settings to generate candidates.".to_string()
                })?;
            llm::detect_candidates_with_openai(&normalized, &key)
                .await
                .map_err(|e| e.to_string())?
        }
        "openrouter" => {
            let key = credentials::get(credentials::OPENROUTER)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    "Configure an OpenRouter API key in Settings to generate candidates."
                        .to_string()
                })?;
            llm::detect_candidates_with_openrouter(&normalized, &key, model_name.as_deref())
                .await
                .map_err(|e| e.to_string())?
        }
        "groq" => {
            let key = credentials::get(credentials::GROQ)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    "Configure a Groq API key in Settings to generate candidates.".to_string()
                })?;
            llm::detect_candidates_with_groq(&normalized, &key)
                .await
                .map_err(|e| e.to_string())?
        }
        _ => {
            let key = credentials::get(credentials::DEEPSEEK)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    "Configure a DeepSeek API key in Settings to generate candidates.".to_string()
                })?;
            llm::detect_candidates_with_deepseek(&normalized, &key, model_name.as_deref())
                .await
                .map_err(|e| e.to_string())?
        }
    };

    if drafts.is_empty() {
        return Err("No viable clip candidates were returned for this transcript.".to_string());
    }

    // Pro-Editor Pipeline:
    // 1. Algorithmic Sentence & Context Boundary Snapping
    let snapped_drafts = pro_editor::snap_candidates_to_boundaries(&drafts, &normalized);

    // 2. Multi-Modal Audio Energy & Viral Hook Scoring
    let wav_path = state
        .data_dir
        .join("projects")
        .join(project_id)
        .join("transcription_audio.wav");
    let wav_opt = if wav_path.exists() {
        Some(wav_path.as_path())
    } else {
        None
    };
    let scored_drafts =
        pro_editor::calculate_audio_energy_scores(wav_opt, &snapped_drafts, &normalized);

    let candidates = db
        .replace_candidates(project_id, &scored_drafts)
        .map_err(|e| e.to_string())?;
    db.update_project_status(project_id, "ready", None)
        .map_err(|e| e.to_string())?;
    Ok(candidates)
}

pub fn set_selected_clip_count(
    state: &AppState,
    project_id: &str,
    count: usize,
) -> Result<Vec<Candidate>, String> {
    state
        .db
        .set_selected_clip_count(project_id, count.clamp(0, 10))
        .map_err(|e| e.to_string())
}

pub async fn update_candidate_timing(
    state: &AppState,
    candidate_id: &str,
    start_sec: f64,
    end_sec: f64,
) -> Result<Candidate, String> {
    let db = state.db.clone();
    let valid_start = start_sec.max(0.0);
    let valid_end = end_sec.max(valid_start + 1.0);
    db.update_candidate_timing(candidate_id, valid_start, valid_end)
        .map_err(|e| e.to_string())?;
    let (candidate, _) = db
        .get_candidate_with_project(candidate_id)
        .map_err(|e| e.to_string())?;
    Ok(candidate)
}

pub async fn update_candidate_layout_override(
    state: &AppState,
    candidate_id: &str,
    layout_override: Option<String>,
) -> Result<Candidate, String> {
    let db = state.db.clone();
    let sanitized_override = layout_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "auto");
    db.update_candidate_layout_override(candidate_id, sanitized_override)
        .map_err(|e| e.to_string())?;
    let (candidate, _) = db
        .get_candidate_with_project(candidate_id)
        .map_err(|e| e.to_string())?;
    Ok(candidate)
}

pub async fn generate_social_kit_for_candidate(
    state: &AppState,
    candidate_id: &str,
    provider: Option<String>,
    model_name: Option<String>,
) -> Result<SocialKit, String> {
    let db = state.db.clone();
    let (candidate, project) = db
        .get_candidate_with_project(candidate_id)
        .map_err(|e| e.to_string())?;

    let mut transcript_text = String::new();
    if let Ok(Some(transcript_record)) = db.latest_transcript(&project.id) {
        if let Ok(normalized) =
            serde_json::from_str::<NormalizedTranscript>(&transcript_record.raw_json)
        {
            let words: Vec<&str> = normalized
                .words
                .iter()
                .filter(|w| w.end >= candidate.start_sec && w.start <= candidate.end_sec)
                .map(|w| w.text.as_str())
                .collect();
            transcript_text = words.join(" ");
        }
    }

    let kit = llm::generate_social_kit(
        &candidate.id,
        &candidate.hook,
        &transcript_text,
        provider.as_deref(),
        model_name.as_deref(),
    )
    .await;
    Ok(kit)
}
