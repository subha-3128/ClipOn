use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstagramConfig {
    pub enabled: bool,
    pub min_score: f64,   // e.g. 0.90 for 90%
    pub provider: String, // "graph_api", "webhook"
    pub account_id: Option<String>,
    pub access_token: Option<String>,
    pub webhook_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InitMediaResponse {
    id: String,
    #[serde(alias = "upload_url")]
    uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MetaErrorPayload {
    message: Option<String>,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    error_type: Option<String>,
    code: Option<i64>,
    error_subcode: Option<i64>,
    error_user_title: Option<String>,
    error_user_msg: Option<String>,
    #[allow(dead_code)]
    fbtrace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MetaErrorResponse {
    error: MetaErrorPayload,
}

/// Parse and translate Meta Graph API error payloads into clear, actionable messages.
pub fn format_meta_api_error(err_text: &str) -> String {
    if let Ok(resp) = serde_json::from_str::<MetaErrorResponse>(err_text) {
        let code = resp.error.code.unwrap_or(0);
        let subcode = resp.error.error_subcode.unwrap_or(0);
        let raw_msg = resp
            .error
            .message
            .unwrap_or_else(|| "Unknown Meta API error".to_string());

        if let Some(user_msg) = resp.error.error_user_msg.filter(|m| !m.trim().is_empty()) {
            if let Some(user_title) = resp.error.error_user_title.filter(|t| !t.trim().is_empty()) {
                return format!("{user_title}: {user_msg} (Meta Code: {code})");
            }
            return format!("{user_msg} (Meta Code: {code})");
        }

        match (code, subcode) {
            (190, 463) => "Your Meta Page Access Token has expired. Please generate a new Page Access Token in Meta Graph API Explorer or Meta Business Suite.".to_string(),
            (190, 467) => "Invalid access token: Token was revoked or invalidated. Please refresh your credentials in Settings.".to_string(),
            (190, 490) => "Authorization scope missing: Ensure your token has 'instagram_content_publish' and 'instagram_basic' permissions enabled.".to_string(),
            (190, _) => format!("Meta Authentication failed: {raw_msg}. Ensure you are using a Meta Page Access Token (starts with 'EAA...')."),
            (10, _) | (200, _) => "Permission Denied: Ensure your Instagram account is a Professional (Creator or Business) account and is linked to the authorized Facebook Page.".to_string(),
            (100, 2207001) => "Instagram Reel video duration error: Reels must be between 3 seconds and 15 minutes in length.".to_string(),
            (100, 2207003) => "Unsupported video aspect ratio: Instagram Reels require a 9:16 vertical ratio (e.g., 1080x1920).".to_string(),
            (100, _) => format!("Instagram API parameter error: {raw_msg} (Code: 100). Check that your Instagram Account ID is correct."),
            (24, _) | (4, _) => "Instagram API rate limit reached (maximum 50 posts per 24 hours per account). Please try again later.".to_string(),
            (352, _) | (353, _) => "Instagram media processing failed on Meta transcoding nodes. Verify that the video is valid H.264 MP4 with AAC audio.".to_string(),
            _ => format!("{raw_msg} (Meta Code: {code}, Subcode: {subcode})"),
        }
    } else {
        err_text.trim().to_string()
    }
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    status_code: Option<String>,
    status: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublishResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct PermalinkResponse {
    permalink: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct UserResponse {
    id: Option<String>,
    username: Option<String>,
    name: Option<String>,
    account_type: Option<String>,
}

/// Helper to sanitize token input: trims whitespace, quotes, and Bearer / OAuth prefixes.
pub fn sanitize_token(token: &str) -> String {
    let mut s = token.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s = s[1..s.len() - 1].trim();
    }
    if let Some(rest) = s.strip_prefix("Bearer ") {
        s = rest.trim();
    } else if let Some(rest) = s.strip_prefix("bearer ") {
        s = rest.trim();
    } else if let Some(rest) = s.strip_prefix("OAuth ") {
        s = rest.trim();
    } else if let Some(rest) = s.strip_prefix("oauth ") {
        s = rest.trim();
    }
    s.to_string()
}

pub fn is_instagram_user_token(token: &str) -> bool {
    let t = token.trim();
    t.starts_with("IGA") || t.starts_with("IGQ")
}

/// Test connection to Instagram (Graph API or Webhook)
pub async fn test_connection(
    provider: &str,
    account_id: Option<&str>,
    access_token: Option<&str>,
    webhook_url: Option<&str>,
) -> Result<String> {
    match provider {
        "webhook" => {
            let url = webhook_url
                .filter(|u| !u.trim().is_empty())
                .ok_or_else(|| anyhow!("Webhook URL is required"))?;

            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let res = client
                .post(url)
                .json(&json!({
                    "event": "clipon_ping",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "message": "ClipOn Instagram Webhook Test"
                }))
                .send()
                .await
                .map_err(|e| anyhow!("Failed to send ping to webhook: {e}"))?;

            if res.status().is_success() {
                Ok("Webhook connected successfully! (Status 200 OK)".to_string())
            } else {
                Err(anyhow!("Webhook returned error status: {}", res.status()))
            }
        }
        _ => {
            let raw_token = access_token
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| anyhow!("Meta Access Token is required"))?;
            let token = sanitize_token(raw_token);
            if token.is_empty() {
                return Err(anyhow!("Meta Access Token cannot be empty"));
            }

            let is_ig_platform = is_instagram_user_token(&token);
            let url = if is_ig_platform {
                "https://graph.instagram.com/v20.0/me?fields=id,username,name,account_type"
                    .to_string()
            } else {
                let user_id = account_id
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| anyhow!("Instagram Account ID is required"))?;
                format!(
                    "https://graph.facebook.com/v20.0/{}?fields=id,username,name",
                    user_id.trim()
                )
            };

            let res = reqwest::Client::new()
                .get(&url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .map_err(|e| anyhow!("Network error verifying Instagram credentials: {e}"))?;

            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                let formatted = format_meta_api_error(&err_text);
                return Err(anyhow!("Instagram authentication failed: {formatted}"));
            }

            let user: UserResponse = res
                .json()
                .await
                .map_err(|e| anyhow!("Failed to parse Instagram profile response: {e}"))?;

            let username = user.username.unwrap_or_else(|| "creator".to_string());
            let acc_type = user
                .account_type
                .map(|t| format!(" ({t})"))
                .unwrap_or_default();

            if is_ig_platform {
                Ok(format!(
                    "Connected successfully to Instagram as @{username}{acc_type} (Instagram User Account)"
                ))
            } else {
                Ok(format!(
                    "Connected successfully to Instagram as @{username}{acc_type} (Meta Page Account)"
                ))
            }
        }
    }
}

const STAGING_STORAGE_BUCKET: &str = "reels-upload";
const DEFAULT_SUPABASE_URL: &str = "https://prblywfioszvmjvydpyp.supabase.co";
const DEFAULT_SUPABASE_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InByYmx5d2Zpb3N6dm1qdnlkcHlwIiwicm9sZSI6ImFub24iLCJpYXQiOjE3OTAzMTkxMTcsImV4cCI6MjEwNTg5NTExN30.xo3pgdb10NsEogv40Cn3amViOAODQhzDZlbIC2okYYk";

/// Publish video to Instagram Reels using the Instagram Platform API (for Instagram User Tokens: IGAA...)
async fn publish_reel_instagram_platform(
    account_id: &str,
    access_token: &str,
    video_path: &str,
    caption: &str,
) -> Result<String> {
    let path = Path::new(video_path);
    if !path.exists() {
        return Err(anyhow!("Video file does not exist at: {video_path}"));
    }

    let file_bytes = tokio::fs::read(path)
        .await
        .map_err(|e| anyhow!("Failed to read video file: {e}"))?;

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let supabase_url = std::env::var("SUPABASE_URL")
        .unwrap_or_else(|_| DEFAULT_SUPABASE_URL.to_string());
    let supabase_key = std::env::var("SUPABASE_ANON_KEY")
        .unwrap_or_else(|_| DEFAULT_SUPABASE_KEY.to_string());

    let object_name = format!("{}.mp4", Uuid::new_v4());
    let staging_upload_url = format!("{}/storage/v1/object/{}/{}", supabase_url, STAGING_STORAGE_BUCKET, object_name);
    let public_video_url = format!("{}/storage/v1/object/public/{}/{}", supabase_url, STAGING_STORAGE_BUCKET, object_name);

    eprintln!("[Instagram] Staging video to public bucket: {}", staging_upload_url);

    // 1. Stage video to high-speed public CDN so Instagram servers can ingest it
    let upload_res = client
        .post(&staging_upload_url)
        .header("apikey", &supabase_key)
        .header("Authorization", format!("Bearer {supabase_key}"))
        .header("Content-Type", "video/mp4")
        .body(file_bytes)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to upload video to staging storage: {e}"))?;

    if !upload_res.status().is_success() {
        let err_text = upload_res.text().await.unwrap_or_default();
        return Err(anyhow!("Video staging upload failed: {err_text}"));
    }

    fn delete_staged_media(url: &str, key: &str) {
        let del_url = url.to_string();
        let auth_key = key.to_string();
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let _ = client
                .delete(&del_url)
                .header("apikey", &auth_key)
                .header("Authorization", format!("Bearer {auth_key}"))
                .send()
                .await;
        });
    }

    // 2. Initialize Reel Container on graph.instagram.com
    let init_url = format!("https://graph.instagram.com/v20.0/{account_id}/media");
    let init_res = client
        .post(&init_url)
        .json(&json!({
            "media_type": "REELS",
            "video_url": public_video_url,
            "caption": caption,
            "access_token": access_token
        }))
        .send()
        .await
        .map_err(|e| {
            delete_staged_media(&staging_upload_url, &supabase_key);
            anyhow!("Failed to initiate Instagram media container: {e}")
        })?;

    if !init_res.status().is_success() {
        delete_staged_media(&staging_upload_url, &supabase_key);
        let err = init_res.text().await.unwrap_or_default();
        let formatted = format_meta_api_error(&err);
        return Err(anyhow!("Failed to initialize Instagram Reel: {formatted}"));
    }

    let init_data: InitMediaResponse = init_res
        .json()
        .await
        .map_err(|e| {
            delete_staged_media(&staging_upload_url, &supabase_key);
            anyhow!("Invalid media container response: {e}")
        })?;
    let container_id = init_data.id;
    eprintln!("[Instagram] Container initialized: {}", container_id);

    // 3. Poll Container processing status
    let status_url = format!(
        "https://graph.instagram.com/v20.0/{container_id}?fields=status_code,status,error_message&access_token={access_token}"
    );

    let mut attempts = 0;
    let mut is_ready = false;
    while attempts < 80 {
        sleep(Duration::from_secs(3)).await;
        attempts += 1;

        if let Ok(res) = client.get(&status_url).send().await {
            if let Ok(st) = res.json::<StatusResponse>().await {
                if let Some(code) = st.status_code.as_deref() {
                    match code {
                        "FINISHED" => {
                            is_ready = true;
                            eprintln!("[Instagram] Container {container_id} is FINISHED and ready!");
                            break;
                        }
                        "ERROR" => {
                            delete_staged_media(&staging_upload_url, &supabase_key);
                            let err_detail = st.error_message.unwrap_or_else(|| {
                                st.status.unwrap_or_else(|| "Instagram processing rejected video".to_string())
                            });
                            let formatted = format_meta_api_error(&err_detail);
                            return Err(anyhow!("Instagram processing error: {formatted}"));
                        }
                        "EXPIRED" => {
                            delete_staged_media(&staging_upload_url, &supabase_key);
                            return Err(anyhow!("Instagram container upload session expired"));
                        }
                        _ => {
                            eprintln!("[Instagram] Container {container_id} status: {code} (attempt {attempts}/80)");
                        }
                    }
                }
            }
        }
    }

    if !is_ready {
        delete_staged_media(&staging_upload_url, &supabase_key);
        return Err(anyhow!("Instagram video processing timed out after 4 minutes"));
    }

    // 4. Publish Media
    let publish_url = format!("https://graph.instagram.com/v20.0/{account_id}/media_publish");
    let pub_res = client
        .post(&publish_url)
        .json(&json!({
            "creation_id": container_id,
            "access_token": access_token
        }))
        .send()
        .await
        .map_err(|e| {
            delete_staged_media(&staging_upload_url, &supabase_key);
            anyhow!("Failed to send publish command to Instagram: {e}")
        })?;

    if !pub_res.status().is_success() {
        delete_staged_media(&staging_upload_url, &supabase_key);
        let err = pub_res.text().await.unwrap_or_default();
        let formatted = format_meta_api_error(&err);
        return Err(anyhow!("Instagram Reel publication failed: {formatted}"));
    }

    let pub_data: PublishResponse = pub_res
        .json()
        .await
        .map_err(|e| {
            delete_staged_media(&staging_upload_url, &supabase_key);
            anyhow!("Invalid publish response from Instagram: {e}")
        })?;

    let media_id = pub_data.id;
    eprintln!("[Instagram] Media published successfully! ID: {}", media_id);

    // 5. Clean up temporary staging asset
    delete_staged_media(&staging_upload_url, &supabase_key);

    // 6. Retrieve live permalink
    let permalink_url = format!(
        "https://graph.instagram.com/v20.0/{media_id}?fields=permalink&access_token={access_token}"
    );

    if let Ok(res) = client.get(&permalink_url).send().await {
        if let Ok(data) = res.json::<PermalinkResponse>().await {
            if let Some(link) = data.permalink {
                return Ok(link);
            }
        }
    }

    Ok(format!("https://www.instagram.com/reel/{media_id}/"))
}

/// Publish video to Instagram Reels using official Meta Graph API (Direct Resumable Upload)
pub async fn publish_reel_graph_api(
    account_id: &str,
    access_token: &str,
    video_path: &str,
    caption: &str,
) -> Result<String> {
    let clean_token = sanitize_token(access_token);
    if clean_token.is_empty() {
        return Err(anyhow!("Instagram Meta Access Token is required"));
    }

    let clean_account_id = account_id.trim();
    if clean_account_id.is_empty() {
        return Err(anyhow!("Instagram Account ID is required for Graph API publishing"));
    }

    // If using an Instagram User Token (IGAA...), publish via Instagram Platform API
    if is_instagram_user_token(&clean_token) {
        return publish_reel_instagram_platform(clean_account_id, &clean_token, video_path, caption).await;
    }

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let path = Path::new(video_path);
    if !path.exists() {
        return Err(anyhow!("Video file does not exist at: {video_path}"));
    }

    let file_bytes = tokio::fs::read(path)
        .await
        .map_err(|e| anyhow!("Failed to read video file: {e}"))?;
    let file_size = file_bytes.len();

    // 1. Initialize Reel container with official Resumable Upload on Meta Facebook Graph API
    let init_url = format!("https://graph.facebook.com/v20.0/{clean_account_id}/media");
    let init_res = client
        .post(&init_url)
        .header("Authorization", format!("Bearer {clean_token}"))
        .json(&json!({
            "media_type": "REELS",
            "upload_type": "resumable",
            "caption": caption,
            "access_token": clean_token
        }))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to start Instagram media container session: {e}"))?;

    if !init_res.status().is_success() {
        let err = init_res.text().await.unwrap_or_default();
        let formatted = format_meta_api_error(&err);
        return Err(anyhow!("Failed to initialize Instagram Reel upload: {formatted}"));
    }

    let init_data: InitMediaResponse = init_res
        .json()
        .await
        .map_err(|e| anyhow!("Invalid media session response: {e}"))?;

    let container_id = init_data.id;
    let upload_uri = init_data.uri.unwrap_or_else(|| {
        format!("https://rupload.facebook.com/ig-api-upload/v20.0/{container_id}")
    });

    // 2. Direct binary upload to Meta's rupload endpoint with correct video/mp4 MIME type
    let upload_res = client
        .post(&upload_uri)
        .header("Authorization", format!("OAuth {clean_token}"))
        .header("offset", "0")
        .header("file_size", file_size.to_string())
        .header("Content-Type", "video/mp4")
        .body(file_bytes)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to upload video bytes to Instagram: {e}"))?;

    if !upload_res.status().is_success() {
        let err = upload_res.text().await.unwrap_or_default();
        let formatted = format_meta_api_error(&err);
        return Err(anyhow!("Instagram video binary upload failed: {formatted}"));
    }

    // 3. Poll container processing status
    let status_url = format!(
        "https://graph.facebook.com/v20.0/{container_id}?fields=status_code,status,error_message&access_token={clean_token}"
    );

    let mut attempts = 0;
    let mut is_ready = false;
    while attempts < 80 {
        sleep(Duration::from_secs(3)).await;
        attempts += 1;

        if let Ok(res) = client
            .get(&status_url)
            .header("Authorization", format!("Bearer {clean_token}"))
            .send()
            .await
        {
            if let Ok(st) = res.json::<StatusResponse>().await {
                if let Some(code) = st.status_code.as_deref() {
                    match code {
                        "FINISHED" => {
                            is_ready = true;
                            eprintln!(
                                "[Instagram] Container {} is FINISHED and ready to publish!",
                                container_id
                            );
                            break;
                        }
                        "ERROR" => {
                            let err_detail = st.error_message.unwrap_or_else(|| {
                                st.status.unwrap_or_else(|| {
                                    "Instagram rejected video format or processing failed".to_string()
                                })
                            });
                            let formatted = format_meta_api_error(&err_detail);
                            eprintln!(
                                "[Instagram] Container {} error: {}",
                                container_id, formatted
                            );
                            return Err(anyhow!("Instagram processing error: {formatted}"));
                        }
                        "EXPIRED" => {
                            return Err(anyhow!("Instagram container upload session expired (24h timeout)"));
                        }
                        _ => {
                            eprintln!(
                                "[Instagram] Container {} status: {} (attempt {}/80)",
                                container_id, code, attempts
                            );
                        }
                    }
                }
            }
        }
    }

    if !is_ready {
        return Err(anyhow!(
            "Instagram video processing timed out after 4 minutes"
        ));
    }

    // 4. Publish Media
    let publish_url = format!("https://graph.facebook.com/v20.0/{clean_account_id}/media_publish");
    let pub_res = client
        .post(&publish_url)
        .header("Authorization", format!("Bearer {clean_token}"))
        .json(&json!({
            "creation_id": container_id,
            "access_token": clean_token
        }))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send publish command to Instagram: {e}"))?;

    if !pub_res.status().is_success() {
        let err = pub_res.text().await.unwrap_or_default();
        let formatted = format_meta_api_error(&err);
        return Err(anyhow!("Instagram Reel publication failed: {formatted}"));
    }

    let pub_data: PublishResponse = pub_res
        .json()
        .await
        .map_err(|e| anyhow!("Invalid publish response: {e}"))?;

    let media_id = pub_data.id;

    // 5. Retrieve live permalink
    let permalink_url = format!(
        "https://graph.facebook.com/v20.0/{media_id}?fields=permalink&access_token={clean_token}"
    );

    if let Ok(res) = client
        .get(&permalink_url)
        .header("Authorization", format!("Bearer {clean_token}"))
        .send()
        .await
    {
        if let Ok(data) = res.json::<PermalinkResponse>().await {
            if let Some(link) = data.permalink {
                return Ok(link);
            }
        }
    }

    Ok(format!("https://www.instagram.com/reel/{media_id}/"))
}

/// Publish video via Webhook
pub async fn publish_reel_webhook(
    webhook_url: &str,
    candidate_id: &str,
    video_path: &str,
    caption: &str,
    score: f64,
    hook: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let res = client
        .post(webhook_url)
        .json(&json!({
            "event": "publish_reel",
            "candidate_id": candidate_id,
            "video_path": video_path,
            "caption": caption,
            "viral_score": score,
            "hook": hook,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send payload to webhook: {e}"))?;

    if res.status().is_success() {
        Ok("Published via Webhook successfully".to_string())
    } else {
        Err(anyhow!(
            "Webhook endpoint returned status: {}",
            res.status()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_token() {
        assert_eq!(sanitize_token("  my_token  "), "my_token");
        assert_eq!(sanitize_token("\"quoted_token\""), "quoted_token");
        assert_eq!(sanitize_token("'single_quoted'"), "single_quoted");
        assert_eq!(sanitize_token("Bearer EAAB12345"), "EAAB12345");
        assert_eq!(sanitize_token("OAuth EAAB12345"), "EAAB12345");
        assert_eq!(sanitize_token("  \"Bearer EAAB12345\"  "), "EAAB12345");
    }

    #[test]
    fn test_is_instagram_user_token() {
        assert!(is_instagram_user_token("IGAA12345"));
        assert!(is_instagram_user_token("IGQ12345"));
        assert!(!is_instagram_user_token("EAAB12345"));
    }

    #[test]
    fn test_format_meta_api_error_expired_token() {
        let err_json = r#"{"error":{"message":"Session has expired","type":"OAuthException","code":190,"error_subcode":463,"fbtrace_id":"xyz"}}"#;
        let formatted = format_meta_api_error(err_json);
        assert!(formatted.contains("expired"));
        assert!(formatted.contains("Page Access Token"));
    }

    #[test]
    fn test_format_meta_api_error_permission_denied() {
        let err_json = r#"{"error":{"message":"Permission denied","type":"OAuthException","code":200,"fbtrace_id":"xyz"}}"#;
        let formatted = format_meta_api_error(err_json);
        assert!(formatted.contains("Permission Denied"));
        assert!(formatted.contains("Professional"));
    }

    #[test]
    fn test_format_meta_api_error_aspect_ratio() {
        let err_json = r#"{"error":{"message":"Invalid aspect ratio","type":"OAuthException","code":100,"error_subcode":2207003}}"#;
        let formatted = format_meta_api_error(err_json);
        assert!(formatted.contains("9:16"));
    }
}

