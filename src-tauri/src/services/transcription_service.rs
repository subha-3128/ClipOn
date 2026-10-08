use crate::credentials;
use crate::media;
use crate::models::{NormalizedTranscript, Transcript, TranscriptWord};
use crate::services::project_service::project_dir;
use crate::transcription;
use crate::AppState;

pub fn extract_project_audio(state: &AppState, project_id: &str) -> Result<String, String> {
    let project = state
        .db
        .get_project(project_id)
        .map_err(|e| e.to_string())?;
    let audio_path = media::extract_audio(&project.source_path, &project_dir(state, project_id))
        .map_err(|e| e.to_string())?;
    Ok(audio_path.to_string_lossy().to_string())
}

pub async fn transcribe_project(
    state: &AppState,
    project_id: &str,
    provider: &str,
) -> Result<Transcript, String> {
    let db = state.db.clone();
    let data_dir = state.data_dir.clone();
    let project = db.get_project(project_id).map_err(|e| e.to_string())?;
    db.update_project_status(project_id, "transcribing", None)
        .map_err(|e| e.to_string())?;

    let transcript = match provider {
        "deepgram" => {
            let key = credentials::get(credentials::DEEPGRAM)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| {
                    "Configure a Deepgram API key in Settings to use cloud transcription."
                        .to_string()
                })?;
            let audio_path = media::extract_audio(
                &project.source_path,
                &data_dir.join("projects").join(project_id),
            )
            .map_err(|e| e.to_string())?;
            transcription::transcribe_deepgram(&audio_path.to_string_lossy(), &key)
                .await
                .map_err(|e| e.to_string())?
        }
        "local" => {
            let has_whisper =
                transcription::whisper_cli_exists() || transcription::whisper_python_exists();
            if !has_whisper {
                return Err("Whisper is not installed. Please install it (e.g., via Homebrew 'brew install whisper-cli' or via Python 'pip3 install openai-whisper').".to_string());
            }
            let audio_path = media::extract_audio(
                &project.source_path,
                &data_dir.join("projects").join(project_id),
            )
            .map_err(|e| e.to_string())?;
            let whisper_model_env =
                std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "base".to_string());
            let model: transcription::WhisperModel = whisper_model_env
                .parse()
                .unwrap_or(transcription::WhisperModel::Base);
            transcription::transcribe_local_with_model(
                &audio_path.to_string_lossy(),
                &model,
                Some(&data_dir),
            )
            .await
            .map_err(|e| e.to_string())?
        }
        other => return Err(format!("Unsupported transcription provider: {other}")),
    };

    let raw_json = serde_json::to_string_pretty(&transcript).map_err(|e| e.to_string())?;
    let saved = db
        .save_transcript(project_id, provider, &raw_json, Some(&transcript.language))
        .map_err(|e| e.to_string())?;
    db.update_project_status(project_id, "analyzing", Some(transcript.duration))
        .map_err(|e| e.to_string())?;
    Ok(saved)
}

pub fn save_demo_transcript(state: &AppState, project_id: &str) -> Result<Transcript, String> {
    let transcript = demo_transcript();
    let raw_json = serde_json::to_string_pretty(&transcript).map_err(|e| e.to_string())?;
    let saved = state
        .db
        .save_transcript(project_id, "demo", &raw_json, Some(&transcript.language))
        .map_err(|e| e.to_string())?;
    state
        .db
        .update_project_status(project_id, "analyzing", Some(transcript.duration))
        .map_err(|e| e.to_string())?;
    Ok(saved)
}

pub fn demo_transcript() -> NormalizedTranscript {
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
