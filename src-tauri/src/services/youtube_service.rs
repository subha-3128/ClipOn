use crate::credentials;
use crate::models::YouTubePost;
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
    client_id: Option<String>,
    client_secret: Option<String>,
    refresh_token: Option<String>,
) -> Result<YouTubePost, String> {
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

    let title = if let Some(t) = title_override.filter(|s| !s.trim().is_empty()) {
        t
    } else {
        candidate.hook.clone()
    };

    let description = if let Some(d) = description_override.filter(|s| !s.trim().is_empty()) {
        d
    } else {
        format!("{}\n\n#Shorts #ClipOn", candidate.rationale)
    };

    let privacy_status = privacy.unwrap_or_else(|| "public".to_string());
    let tags = vec![
        "Shorts".to_string(),
        "ClipOn".to_string(),
        "viral".to_string(),
    ];

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
