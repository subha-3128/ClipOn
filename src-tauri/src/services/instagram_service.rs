use crate::credentials;
use crate::instagram;
use crate::llm;
use crate::models::{InstagramPost, NormalizedTranscript};
use crate::services::render_service::render_flat_clip_for_candidate;
use crate::AppState;

pub async fn test_instagram_connection(
    provider: &str,
    account_id: Option<String>,
    access_token: Option<String>,
    webhook_url: Option<String>,
) -> Result<String, String> {
    let token = access_token
        .filter(|t| !t.trim().is_empty())
        .or_else(|| credentials::get(credentials::INSTAGRAM).ok().flatten());
    instagram::test_connection(
        provider,
        account_id.as_deref(),
        token.as_deref(),
        webhook_url.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

pub async fn publish_candidate_to_instagram(
    app: tauri::AppHandle,
    state: AppState,
    candidate_id: String,
    caption_override: Option<String>,
    provider: Option<String>,
    account_id: Option<String>,
    access_token: Option<String>,
    webhook_url: Option<String>,
) -> Result<InstagramPost, String> {
    let db = state.db.clone();
    let (candidate, project) = db
        .get_candidate_with_project(&candidate_id)
        .map_err(|e| e.to_string())?;

    let mut clip = db
        .list_clips_for_project(&project.id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.candidate_id == candidate_id);

    let output_path = match &clip {
        Some(c) if c.status == "done" && c.output_path.is_some() => c.output_path.clone().unwrap(),
        _ => {
            // Automatically render clip with 9:16 vertical blur and subtitles
            let rendered = render_flat_clip_for_candidate(
                app,
                state.clone(),
                candidate_id.clone(),
                Some("vertical_crop".to_string()),
                None,
                None,
                None,
                None,
                Some("instagram_reels".to_string()),
            )
            .await
            .map_err(|e| format!("Failed to automatically render clip for Instagram: {e}"))?;

            // Refresh clip record from DB so clip_id is properly linked to the Instagram post
            clip = db
                .list_clips_for_project(&project.id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|c| c.candidate_id == candidate_id);

            rendered
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
            if let Ok(normalized) =
                serde_json::from_str::<NormalizedTranscript>(&transcript_record.raw_json)
            {
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
        let kit = llm::generate_social_kit(&candidate.id, &candidate.hook, &transcript_text, None, None).await;
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
                .filter(|t| !t.trim().is_empty())
                .or_else(|| credentials::get(credentials::INSTAGRAM).ok().flatten())
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
                .map_err(|e| e.to_string())?;
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
