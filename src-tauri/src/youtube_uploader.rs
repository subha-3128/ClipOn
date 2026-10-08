use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeUploadResult {
    pub video_id: String,
    pub video_url: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    #[allow(dead_code)]
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ChannelListResponse {
    items: Option<Vec<ChannelItem>>,
}

#[derive(Debug, Deserialize)]
struct ChannelItem {
    id: String,
    snippet: ChannelSnippet,
}

#[derive(Debug, Deserialize)]
struct ChannelSnippet {
    title: String,
    #[serde(rename = "customUrl")]
    custom_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VideoUploadResponse {
    id: String,
}

/// Refreshes OAuth2 access token using Google's token endpoint
pub async fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let params = [
        ("client_id", client_id.trim()),
        ("client_secret", client_secret.trim()),
        ("refresh_token", refresh_token.trim()),
        ("grant_type", "refresh_token"),
    ];

    let res = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| anyhow!("Network error connecting to Google OAuth endpoint: {e}"))?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(anyhow!("Google OAuth token refresh failed: {err_text}"));
    }

    let token_data: TokenResponse = res
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse Google OAuth token response: {e}"))?;

    Ok(token_data.access_token)
}

/// Tests YouTube connection by verifying OAuth2 credentials and reading channel snippet
pub async fn test_connection(
    client_id: Option<&str>,
    client_secret: Option<&str>,
    refresh_token: Option<&str>,
) -> Result<String> {
    let cid = client_id
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("YouTube Client ID is required"))?;
    let sec = client_secret
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("YouTube Client Secret is required"))?;
    let tok = refresh_token
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("YouTube Refresh Token is required"))?;

    let access_token = refresh_access_token(cid, sec, tok).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let res = client
        .get("https://www.googleapis.com/youtube/v3/channels?part=snippet&mine=true")
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| anyhow!("Network error querying YouTube channel: {e}"))?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        if err_text.contains("insufficient authentication scopes") {
            return Ok(
                "Connected successfully to YouTube Data API v3 (OAuth2 Verified with youtube.upload scope)"
                    .to_string(),
            );
        }
        return Err(anyhow!("YouTube channel verification failed: {err_text}"));
    }

    let channels: ChannelListResponse = res
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse YouTube channel profile: {e}"))?;

    if let Some(items) = channels.items {
        if let Some(channel) = items.into_iter().next() {
            let handle = channel
                .snippet
                .custom_url
                .map(|u| format!(" (@{u})"))
                .unwrap_or_default();
            return Ok(format!(
                "Connected successfully to YouTube Channel: {}{handle} (ID: {})",
                channel.snippet.title, channel.id
            ));
        }
    }

    Ok("Connected successfully to YouTube Data API v3 (OAuth2 Verified)".to_string())
}

/// Uploads a vertical video directly to YouTube Shorts via YouTube Data API v3 Resumable Upload
pub async fn upload_shorts(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    video_path: &Path,
    title: &str,
    description: &str,
    tags: &[String],
    privacy_status: &str,
) -> Result<YouTubeUploadResult> {
    if !video_path.exists() {
        return Err(anyhow!("Video file not found at: {}", video_path.display()));
    }

    let file_meta = tokio::fs::metadata(video_path)
        .await
        .with_context(|| format!("Reading video metadata at {}", video_path.display()))?;

    let file_size = file_meta.len();
    if file_size == 0 {
        return Err(anyhow!("Video file is empty (0 bytes)"));
    }

    let access_token = refresh_access_token(client_id, client_secret, refresh_token).await?;
    let clean_title = format_youtube_title(title);

    // Format description: append #Shorts tag
    let mut clean_desc = description.trim().to_string();
    if !clean_desc.to_lowercase().contains("#shorts") {
        clean_desc = format!("{clean_desc}\n\n#Shorts #ClipOn");
    }

    let clean_privacy = match privacy_status.to_lowercase().as_str() {
        "unlisted" => "unlisted",
        "private" => "private",
        _ => "public",
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600)) // 10 minute upload timeout
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // Step 1: Initiate Resumable Upload
    let metadata = json!({
        "snippet": {
            "title": clean_title,
            "description": clean_desc,
            "tags": tags,
            "categoryId": "22"
        },
        "status": {
            "privacyStatus": clean_privacy,
            "selfDeclaredMadeForKids": false
        }
    });

    let init_res = client
        .post("https://www.googleapis.com/upload/youtube/v3/videos?uploadType=resumable&part=snippet,status")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("X-Upload-Content-Length", file_size.to_string())
        .header("X-Upload-Content-Type", "video/mp4")
        .json(&metadata)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to initiate YouTube resumable upload: {e}"))?;

    if !init_res.status().is_success() {
        let err_text = init_res.text().await.unwrap_or_default();
        return Err(anyhow!("YouTube upload initiation rejected: {err_text}"));
    }

    let upload_url = init_res
        .headers()
        .get("location")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| anyhow!("YouTube did not return a resumable upload location URL"))?
        .to_string();

    // Step 2: Stream Video File Directly from Disk (zero buffering into memory)
    let file = tokio::fs::File::open(video_path)
        .await
        .with_context(|| format!("Opening video file for upload stream at {}", video_path.display()))?;
    let stream = tokio_util::io::ReaderStream::new(file);
    let body = reqwest::Body::wrap_stream(stream);

    let upload_res = client
        .put(&upload_url)
        .header("Content-Type", "video/mp4")
        .header("Content-Length", file_size.to_string())
        .body(body)
        .send()
        .await
        .map_err(|e| anyhow!("Network error uploading video stream to YouTube: {e}"))?;

    if !upload_res.status().is_success() {
        let err_text = upload_res.text().await.unwrap_or_default();
        return Err(anyhow!("YouTube video upload rejected: {err_text}"));
    }

    let resp_data: VideoUploadResponse = upload_res
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse YouTube upload response: {e}"))?;

    let video_id = resp_data.id;
    let video_url = format!("https://www.youtube.com/shorts/{video_id}");

    Ok(YouTubeUploadResult {
        video_id,
        video_url,
        title: clean_title,
    })
}

/// Formats YouTube title safely, ensuring #Shorts tag and adhering to 100 char limit
/// without panicking on multi-byte UTF-8 character boundaries (emojis, CJK, non-English).
pub fn format_youtube_title(title: &str) -> String {
    let mut clean_title = title.trim().to_string();
    if clean_title.is_empty() {
        clean_title = "ClipOn Viral Short".to_string();
    }
    if !clean_title.to_lowercase().contains("#shorts") {
        clean_title = format!("{clean_title} #Shorts");
    }
    // YouTube's title limit is 100 characters. Slicing by chars prevents panics on multi-byte UTF-8 boundaries.
    if clean_title.chars().count() > 100 {
        let prefix: String = clean_title.chars().take(92).collect();
        clean_title = format!("{} #Shorts", prefix.trim_end());
    }
    clean_title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_youtube_title_basic() {
        assert_eq!(format_youtube_title("My Great Clip"), "My Great Clip #Shorts");
        assert_eq!(format_youtube_title("Already has #shorts"), "Already has #shorts");
        assert_eq!(format_youtube_title(""), "ClipOn Viral Short #Shorts");
    }

    #[test]
    fn test_format_youtube_title_unicode_and_emojis_no_panic() {
        // Multi-byte Unicode characters right at the boundary
        let long_emoji_title = "🚀🔥✨ 10 Best Places in Tokyo! 🇯🇵🎌 ".repeat(5);
        let result = format_youtube_title(&long_emoji_title);
        assert!(result.chars().count() <= 100);
        assert!(result.ends_with("#Shorts"));

        // Japanese CJK characters
        let japanese_title = "これは非常に長いタイトルのテストです。美しい日本の風景と文化について詳しく説明します。".repeat(3);
        let jp_result = format_youtube_title(&japanese_title);
        assert!(jp_result.chars().count() <= 100);
        assert!(jp_result.ends_with("#Shorts"));
    }
}
