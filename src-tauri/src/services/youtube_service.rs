use crate::credentials;
use crate::models::{NormalizedTranscript, YouTubePost};
use crate::services::render_service::render_flat_clip_for_candidate;
use crate::youtube_uploader;
use crate::AppState;
use std::path::Path;

pub async fn test_youtube_connection(
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
) -> Result<String, String> {
    let cid = client_id.filter(|s| !s.trim().is_empty()).or_else(|| {
        credentials::get(credentials::YOUTUBE_CLIENT_ID)
            .ok()
            .flatten()
    });
    let sec = client_secret.filter(|s| !s.trim().is_empty()).or_else(|| {
        credentials::get(credentials::YOUTUBE_CLIENT_SECRET)
            .ok()
            .flatten()
    });
    let tok = refresh_token.filter(|s| !s.trim().is_empty()).or_else(|| {
        credentials::get(credentials::YOUTUBE_REFRESH_TOKEN)
            .ok()
            .flatten()
    });

    youtube_uploader::test_connection(cid.as_deref(), sec.as_deref(), tok.as_deref())
        .await
        .map_err(|e| e.to_string())
}

pub async fn publish_candidate_to_youtube(
    app: tauri::AppHandle,
    state: AppState,
    candidate_id: String,
    title_override: Option<String>,
    description_override: Option<String>,
    privacy: Option<String>,
    tags_override: Option<Vec<String>>,
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
) -> Result<YouTubePost, String> {
    let db = state.db.clone();
    let (candidate, project) = db
        .get_candidate_with_project(&candidate_id)
        .map_err(|e| e.to_string())?;

    // Check duplicate-post / in-flight publishing protection
    if let Ok(existing_posts) = db.list_youtube_posts_for_project(&project.id) {
        if existing_posts
            .iter()
            .any(|p| p.candidate_id == candidate_id && p.status == "publishing")
        {
            return Err(
                "A publishing operation is already in progress for this Short. Please wait for YouTube to finish processing."
                    .to_string(),
            );
        }
    }

    let mut clip = db
        .list_clips_for_project(&project.id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.candidate_id == candidate_id);

    let output_path = match &clip {
        Some(c) if c.status == "done" && c.output_path.is_some() => c.output_path.clone().unwrap(),
        _ => {
            // Automatically render clip with 9:16 vertical framing and stylized subtitles
            let rendered = render_flat_clip_for_candidate(
                app,
                state.clone(),
                candidate_id.clone(),
                Some("vertical_crop".to_string()),
                None,
                None,
                None,
                None,
                Some("youtube_shorts".to_string()),
            )
            .await
            .map_err(|e| format!("Failed to automatically render clip for YouTube Shorts: {e}"))?;

            // Refresh clip record from DB
            clip = db
                .list_clips_for_project(&project.id)
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|c| c.candidate_id == candidate_id);

            rendered
        }
    };

    let client_id = client_id
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            credentials::get(credentials::YOUTUBE_CLIENT_ID)
                .ok()
                .flatten()
        })
        .ok_or_else(|| "YouTube Client ID not configured. Set in Settings or .env.".to_string())?;
    let client_secret = client_secret
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            credentials::get(credentials::YOUTUBE_CLIENT_SECRET)
                .ok()
                .flatten()
        })
        .ok_or_else(|| {
            "YouTube Client Secret not configured. Set in Settings or .env.".to_string()
        })?;
    let refresh_token = refresh_token
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            credentials::get(credentials::YOUTUBE_REFRESH_TOKEN)
                .ok()
                .flatten()
        })
        .ok_or_else(|| {
            "YouTube Refresh Token not configured. Set in Settings or .env.".to_string()
        })?;

    // Load or generate AI Social Kit for the candidate
    let kit = if let Some(ref existing) = candidate.social_kit {
        existing.clone()
    } else {
        let mut transcript_text = candidate.hook.clone();
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
                if !words.is_empty() {
                    transcript_text = words.join(" ");
                }
            }
        }
        let generated = crate::llm::generate_social_kit(
            &candidate.id,
            &candidate.hook,
            &transcript_text,
            None,
            None,
        )
        .await;
        if let Ok(json_str) = serde_json::to_string(&generated) {
            let _ = db.update_candidate_social_kit(&candidate.id, &json_str);
        }
        generated
    };

    // 1. YouTube Shorts Title: Use override if specified, otherwise pick top AI title or hook
    let title = if let Some(t) = title_override.filter(|s| !s.trim().is_empty()) {
        youtube_uploader::format_youtube_title(&t)
    } else {
        let raw_title = kit
            .titles
            .first()
            .cloned()
            .or_else(|| kit.caption_options.first().map(|o| o.hook.clone()))
            .unwrap_or_else(|| candidate.hook.clone());
        youtube_uploader::format_youtube_title(&raw_title)
    };

    // 2. YouTube Shorts Description: Use override if specified, otherwise combine AI description & hashtags
    let description = if let Some(d) = description_override.filter(|s| !s.trim().is_empty()) {
        d
    } else {
        let tags_str = kit
            .hashtags
            .iter()
            .map(|h| {
                if h.starts_with('#') {
                    h.clone()
                } else {
                    format!("#{h}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let caption_body = kit
            .caption_options
            .first()
            .map(|o| o.text.as_str())
            .unwrap_or(kit.description.as_str());
        if caption_body.is_empty() || caption_body == kit.description {
            format!("{}\n\n#Shorts {}", kit.description, tags_str)
                .trim()
                .to_string()
        } else {
            format!("{}\n\n{}\n\n#Shorts {}", caption_body, kit.description, tags_str)
                .trim()
                .to_string()
        }
    };

    let privacy_status = privacy.unwrap_or_else(|| "public".to_string());

    // 3. YouTube Tags: Use tags_override or AI Social Kit hashtags sanitized without '#'
    let mut tags: Vec<String> = if let Some(user_tags) = tags_override.filter(|t| !t.is_empty()) {
        user_tags
            .into_iter()
            .map(|h| h.trim_start_matches('#').trim().to_string())
            .filter(|h| !h.is_empty())
            .collect()
    } else {
        kit.hashtags
            .iter()
            .map(|h| h.trim_start_matches('#').trim().to_string())
            .filter(|h| !h.is_empty())
            .collect()
    };
    if !tags.iter().any(|t| t.eq_ignore_ascii_case("shorts")) {
        tags.insert(0, "Shorts".to_string());
    }
    if !tags.iter().any(|t| t.eq_ignore_ascii_case("clipon")) {
        tags.push("ClipOn".to_string());
    }

    let _ = db.upsert_youtube_post(
        &candidate_id,
        clip.as_ref().map(|c| c.id.as_str()),
        "publishing",
        Some(&title),
        None,
        None,
        None,
    );

    let upload_result = youtube_uploader::upload_shorts(
        &client_id,
        &client_secret,
        &refresh_token,
        Path::new(&output_path),
        &title,
        &description,
        &tags,
        &privacy_status,
    )
    .await;

    match upload_result {
        Ok(res) => {
            let updated = db
                .upsert_youtube_post(
                    &candidate_id,
                    clip.as_ref().map(|c| c.id.as_str()),
                    "published",
                    Some(&res.title),
                    Some(&res.video_id),
                    Some(&res.video_url),
                    None,
                )
                .map_err(|e| e.to_string())?;
            Ok(updated)
        }
        Err(err) => {
            let err_msg = err.to_string();
            let _ = db.upsert_youtube_post(
                &candidate_id,
                clip.as_ref().map(|c| c.id.as_str()),
                "failed",
                Some(&title),
                None,
                None,
                Some(&err_msg),
            );
            Err(err_msg)
        }
    }
}
