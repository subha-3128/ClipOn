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

    // 2. Compute or Fetch Cached Active Speaker Timeline (analyzed once, cached for all candidate reels)
    let project_opt = db.get_project(project_id).ok();
    let asd_timeline = if let Some(ref proj) = project_opt {
        let probe = crate::media::probe_media(&proj.source_path).ok();
        let total_dur = probe
            .and_then(|p| p.duration_sec)
            .unwrap_or(normalized.duration);
        Some(
            crate::media::get_or_compute_active_speaker_timeline(
                &proj.source_path,
                0.0,
                total_dur,
                Some(&normalized),
            )
            .await,
        )
    } else {
        None
    };

    // 3. Multi-Modal Audio Energy, Visual Quality & Viral Hook Scoring
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
    let mut scored_drafts = pro_editor::calculate_composite_reel_scores(
        wav_opt,
        &snapped_drafts,
        &normalized,
        asd_timeline.as_ref(),
    );

    // Re-rank candidates by final composite score descending so Rank #1 is the highest-scoring hook & retention clip
    scored_drafts.sort_by(|a, b| b.score.total_cmp(&a.score));

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
    if !start_sec.is_finite() || !end_sec.is_finite() {
        return Err("Start and end times must be valid finite numbers.".to_string());
    }
    if start_sec < 0.0 {
        return Err("Start time cannot be negative.".to_string());
    }
    if end_sec <= start_sec {
        return Err("End time must be strictly greater than start time.".to_string());
    }
    let duration = end_sec - start_sec;
    if duration < 3.0 {
        return Err(format!(
            "Clip duration too short: {duration:.1}s. Minimum duration is 3.0 seconds."
        ));
    }
    if duration > 60.0 {
        return Err(format!(
            "Clip duration too long: {duration:.1}s. Maximum duration for short-form clips is 60.0 seconds."
        ));
    }

    let db = state.db.clone();
    let (_, project) = db
        .get_candidate_with_project(candidate_id)
        .map_err(|e| e.to_string())?;

    if let Some(source_dur) = project.source_duration {
        if source_dur > 0.0 {
            if start_sec >= source_dur {
                return Err(format!(
                    "Start time ({start_sec:.1}s) exceeds source video duration ({source_dur:.1}s)."
                ));
            }
            if end_sec > source_dur + 0.1 {
                return Err(format!(
                    "End time ({end_sec:.1}s) exceeds source video duration ({source_dur:.1}s)."
                ));
            }
        }
    }

    db.update_candidate_timing(candidate_id, start_sec, end_sec)
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

    // Persist social kit to SQLite so it survives restarts
    if let Ok(json_str) = serde_json::to_string(&kit) {
        let _ = db.update_candidate_social_kit(&candidate.id, &json_str);
    }

    Ok(kit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::jobs::JobManager;
    use crate::models::CandidateDraft;

    fn test_app_state() -> AppState {
        let temp_dir =
            std::env::temp_dir().join(format!("clipon_cand_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let db_file = temp_dir.join("test.sqlite");
        let db = Database::open(&db_file).unwrap();
        AppState {
            db,
            data_dir: temp_dir,
            jobs: JobManager::new(),
        }
    }

    #[tokio::test]
    async fn test_update_candidate_timing_validation_rules() {
        let state = test_app_state();
        let proj = state
            .db
            .create_project("/dummy/sample.mp4", "local", "modern-box", Some(50.0))
            .expect("create project");

        let drafts = vec![CandidateDraft {
            start: 10.0,
            end: 25.0,
            score: 9.0,
            hook: "Test Hook".into(),
            rationale: "Test Rationale".into(),
            quality_score: None,
        }];

        let candidates = state.db.replace_candidates(&proj.id, &drafts).unwrap();
        let cand_id = &candidates[0].id;

        // Valid range within bounds
        let valid_res = update_candidate_timing(&state, cand_id, 12.0, 30.0).await;
        assert!(valid_res.is_ok());
        let updated = valid_res.unwrap();
        assert_eq!(updated.start_sec, 12.0);
        assert_eq!(updated.end_sec, 30.0);

        // NaN validation
        let nan_res = update_candidate_timing(&state, cand_id, f64::NAN, 30.0).await;
        assert!(nan_res.is_err());
        assert!(nan_res.unwrap_err().contains("finite numbers"));

        // Infinity validation
        let inf_res = update_candidate_timing(&state, cand_id, 10.0, f64::INFINITY).await;
        assert!(inf_res.is_err());

        // Negative start
        let neg_res = update_candidate_timing(&state, cand_id, -2.0, 20.0).await;
        assert!(neg_res.is_err());
        assert!(neg_res.unwrap_err().contains("negative"));

        // Inverted or equal start & end
        let inv_res = update_candidate_timing(&state, cand_id, 25.0, 20.0).await;
        assert!(inv_res.is_err());
        assert!(inv_res.unwrap_err().contains("strictly greater"));

        // Minimum duration (< 3.0s)
        let short_res = update_candidate_timing(&state, cand_id, 10.0, 12.0).await;
        assert!(short_res.is_err());
        assert!(short_res
            .unwrap_err()
            .contains("Minimum duration is 3.0 seconds"));

        // Maximum duration (> 60.0s)
        let long_res = update_candidate_timing(&state, cand_id, 5.0, 70.0).await;
        assert!(long_res.is_err());
        assert!(long_res.unwrap_err().contains("60.0 seconds"));

        // Exceeds video duration (source_duration is 50.0s)
        let exceed_start = update_candidate_timing(&state, cand_id, 55.0, 59.0).await;
        assert!(exceed_start.is_err());
        assert!(exceed_start
            .unwrap_err()
            .contains("exceeds source video duration"));

        let exceed_end = update_candidate_timing(&state, cand_id, 40.0, 52.0).await;
        assert!(exceed_end.is_err());
        assert!(exceed_end
            .unwrap_err()
            .contains("exceeds source video duration"));
    }
}
