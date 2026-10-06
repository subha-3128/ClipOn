use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use tokio::time::{sleep, Duration};

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
    uri: Option<String>,
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

#[derive(Debug, Deserialize)]
struct UguuFile {
    url: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UguuResponse {
    success: bool,
    files: Option<Vec<UguuFile>>,
}

/// Helper: Upload local video to a temporary public HTTPS stream URL for Meta to ingest
async fn upload_to_temp_stream_url(client: &reqwest::Client, video_path: &Path) -> Result<String> {
    let file_bytes = tokio::fs::read(video_path)
        .await
        .map_err(|e| anyhow!("Failed to read video file at {:?}: {e}", video_path))?;

    let filename = video_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("clip.mp4")
        .to_string();

    eprintln!(
        "[Instagram] Uploading video for Meta Reels ingest: {} ({} bytes)",
        filename,
        file_bytes.len()
    );

    // 1. Primary: Uguu.se (Proven compatibility with Meta video ingester)
    let uguu_part = reqwest::multipart::Part::bytes(file_bytes.clone())
        .file_name(filename.clone())
        .mime_str("video/mp4")?;

    let uguu_form = reqwest::multipart::Form::new().part("files[]", uguu_part);

    match client
        .post("https://uguu.se/upload.php")
        .multipart(uguu_form)
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => {
            if let Ok(uguu) = res.json::<UguuResponse>().await {
                if let Some(files) = uguu.files {
                    if let Some(first) = files.first() {
                        eprintln!("[Instagram] Uguu upload successful: {}", first.url);
                        return Ok(first.url.clone());
                    }
                }
            }
        }
        Ok(res) => {
            eprintln!(
                "[Instagram] Uguu upload failed with status: {}",
                res.status()
            );
        }
        Err(e) => {
            eprintln!("[Instagram] Uguu request error: {}", e);
        }
    }

    // 2. Secondary fallback: tmpfiles.org
    #[derive(Deserialize)]
    struct TmpData {
        url: Option<String>,
    }
    #[derive(Deserialize)]
    struct TmpResponse {
        data: Option<TmpData>,
    }

    let tmp_part = reqwest::multipart::Part::bytes(file_bytes.clone())
        .file_name(filename.clone())
        .mime_str("video/mp4")?;
    let tmp_form = reqwest::multipart::Form::new().part("file", tmp_part);

    match client
        .post("https://tmpfiles.org/api/v1/upload")
        .multipart(tmp_form)
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => {
            if let Ok(json_res) = res.json::<TmpResponse>().await {
                if let Some(data) = json_res.data {
                    if let Some(raw_url) = data.url {
                        let direct_url =
                            raw_url.replace("https://tmpfiles.org/", "https://tmpfiles.org/dl/");
                        eprintln!("[Instagram] tmpfiles upload successful: {}", direct_url);
                        return Ok(direct_url);
                    }
                }
            }
        }
        Ok(res) => {
            eprintln!(
                "[Instagram] tmpfiles upload returned status: {}",
                res.status()
            );
        }
        Err(e) => {
            eprintln!("[Instagram] tmpfiles request error: {}", e);
        }
    }

    // 3. Third fallback: Catbox.moe
    let catbox_part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name(filename)
        .mime_str("video/mp4")?;

    let catbox_form = reqwest::multipart::Form::new()
        .text("reqtype", "fileupload")
        .part("fileToUpload", catbox_part);

    match client
        .post("https://catbox.moe/user/api.php")
        .multipart(catbox_form)
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => {
            if let Ok(text) = res.text().await {
                let trimmed = text.trim();
                if trimmed.starts_with("http") {
                    eprintln!("[Instagram] Catbox upload successful: {}", trimmed);
                    return Ok(trimmed.to_string());
                }
            }
        }
        Ok(res) => {
            eprintln!(
                "[Instagram] Catbox upload returned status: {}",
                res.status()
            );
        }
        Err(e) => {
            eprintln!("[Instagram] Catbox request error: {}", e);
        }
    }

    Err(anyhow!(
        "Failed to prepare video for Instagram ingest. Please check internet connection."
    ))
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
            let token = access_token
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| anyhow!("Meta Access Token is required"))?;

            let is_ig_platform = token.trim().starts_with("IGA") || token.trim().starts_with("IGQ");
            let url = if is_ig_platform {
                format!(
                    "https://graph.instagram.com/v20.0/me?fields=id,username,name,account_type&access_token={}",
                    token.trim()
                )
            } else {
                let user_id = account_id
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| anyhow!("Instagram Account ID is required"))?;
                format!(
                    "https://graph.facebook.com/v20.0/{}?fields=id,username,name&access_token={}",
                    user_id.trim(),
                    token.trim()
                )
            };

            let res = reqwest::Client::new()
                .get(&url)
                .send()
                .await
                .map_err(|e| anyhow!("Network error verifying Instagram credentials: {e}"))?;

            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                return Err(anyhow!("Instagram authentication failed: {err_text}"));
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
            Ok(format!(
                "Connected successfully to Instagram as @{username}{acc_type}"
            ))
        }
    }
}

/// Publish video to Instagram Reels using official Meta Graph API
pub async fn publish_reel_graph_api(
    account_id: &str,
    access_token: &str,
    video_path: &str,
    caption: &str,
) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let path = Path::new(video_path);
    if !path.exists() {
        return Err(anyhow!("Video file does not exist at: {video_path}"));
    }

    let is_ig_platform =
        access_token.trim().starts_with("IGA") || access_token.trim().starts_with("IGQ");

    if is_ig_platform {
        // Resolve account ID if missing
        let resolved_account_id = if account_id.trim().is_empty() || account_id == "me" {
            let me_url = format!(
                "https://graph.instagram.com/v20.0/me?fields=id&access_token={}",
                access_token.trim()
            );
            let me_res = client
                .get(&me_url)
                .send()
                .await
                .map_err(|e| anyhow!("Failed to fetch account info: {e}"))?;
            let me_json: serde_json::Value = me_res
                .json()
                .await
                .map_err(|e| anyhow!("Invalid account response: {e}"))?;
            me_json["id"]
                .as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("Failed to resolve Instagram account ID"))?
        } else {
            account_id.trim().to_string()
        };

        // 1. Upload video to stream server to get direct public HTTPS URL
        let stream_url = upload_to_temp_stream_url(&client, path).await?;

        // 2. Initialize Reel container
        let init_url = format!("https://graph.instagram.com/v20.0/{resolved_account_id}/media");
        let init_res = client
            .post(&init_url)
            .form(&[
                ("media_type", "REELS"),
                ("video_url", &stream_url),
                ("caption", caption),
                ("access_token", access_token.trim()),
            ])
            .send()
            .await
            .map_err(|e| anyhow!("Failed to start Instagram media container session: {e}"))?;

        if !init_res.status().is_success() {
            let err = init_res.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to initialize Instagram Reel upload: {err}"));
        }

        let init_data: InitMediaResponse = init_res
            .json()
            .await
            .map_err(|e| anyhow!("Invalid media session response: {e}"))?;

        let container_id = init_data.id;

        // 3. Poll container processing status
        let status_url = format!(
            "https://graph.instagram.com/v20.0/{}?fields=status_code,status,error_message&access_token={}",
            container_id,
            access_token.trim()
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
                                eprintln!(
                                    "[Instagram] Container {} is FINISHED and ready to publish!",
                                    container_id
                                );
                                break;
                            }
                            "ERROR" => {
                                let err_detail = st.error_message.unwrap_or_else(|| {
                                    "Instagram rejected video format or processing failed"
                                        .to_string()
                                });
                                eprintln!(
                                    "[Instagram] Container {} error: {}",
                                    container_id, err_detail
                                );
                                return Err(anyhow!("Instagram processing error: {err_detail}"));
                            }
                            "EXPIRED" => {
                                return Err(anyhow!("Instagram container upload session expired"));
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
        let publish_url = format!(
            "https://graph.instagram.com/v20.0/{}/media_publish?creation_id={}&access_token={}",
            resolved_account_id,
            container_id,
            access_token.trim()
        );

        let pub_res = client
            .post(&publish_url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send publish command to Instagram: {e}"))?;

        if !pub_res.status().is_success() {
            let err = pub_res.text().await.unwrap_or_default();
            return Err(anyhow!("Instagram publishing failed: {err}"));
        }

        let pub_data: PublishResponse = pub_res
            .json()
            .await
            .map_err(|e| anyhow!("Invalid publish response: {e}"))?;

        let media_id = pub_data.id;

        // 5. Retrieve live permalink
        let permalink_url = format!(
            "https://graph.instagram.com/v20.0/{}?fields=permalink&access_token={}",
            media_id,
            access_token.trim()
        );

        if let Ok(res) = client.get(&permalink_url).send().await {
            if let Ok(data) = res.json::<PermalinkResponse>().await {
                if let Some(link) = data.permalink {
                    return Ok(link);
                }
            }
        }

        Ok(format!("https://www.instagram.com/reel/{media_id}"))
    } else {
        // Standard Facebook Login / Meta Resumable Upload
        let file_bytes = tokio::fs::read(path)
            .await
            .map_err(|e| anyhow!("Failed to read video file: {e}"))?;
        let file_size = file_bytes.len();

        let init_url = format!(
            "https://graph.facebook.com/v20.0/{}/media",
            account_id.trim()
        );

        let init_res = client
            .post(&init_url)
            .header("Authorization", format!("Bearer {}", access_token.trim()))
            .json(&json!({
                "media_type": "REELS",
                "upload_type": "resumable",
                "caption": caption
            }))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to start Instagram media container session: {e}"))?;

        if !init_res.status().is_success() {
            let err = init_res.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to initialize Instagram Reel upload: {err}"));
        }

        let init_data: InitMediaResponse = init_res
            .json()
            .await
            .map_err(|e| anyhow!("Invalid media session response: {e}"))?;

        let container_id = init_data.id;
        let upload_uri = init_data.uri.unwrap_or_else(|| {
            format!("https://rupload.facebook.com/reels_upload/{}", container_id)
        });

        let upload_res = client
            .post(&upload_uri)
            .header("Authorization", format!("OAuth {}", access_token.trim()))
            .header("offset", "0")
            .header("file_size", file_size.to_string())
            .header("Content-Type", "application/octet-stream")
            .body(file_bytes)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to upload video bytes to Instagram: {e}"))?;

        if !upload_res.status().is_success() {
            let err = upload_res.text().await.unwrap_or_default();
            return Err(anyhow!("Instagram video binary upload failed: {err}"));
        }

        let status_url = format!(
            "https://graph.facebook.com/v20.0/{}?fields=status_code,status,error_message&access_token={}",
            container_id,
            access_token.trim()
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
                                eprintln!("[Instagram FB] Container {} is FINISHED!", container_id);
                                break;
                            }
                            "ERROR" => {
                                let err_detail = st.error_message.unwrap_or_else(|| {
                                    st.status
                                        .unwrap_or_else(|| "Unknown processing error".to_string())
                                });
                                eprintln!(
                                    "[Instagram FB] Container {} error: {}",
                                    container_id, err_detail
                                );
                                return Err(anyhow!(
                                    "Instagram failed to process video: {err_detail}"
                                ));
                            }
                            _ => {
                                eprintln!(
                                    "[Instagram FB] Container {} status: {} (attempt {}/80)",
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
                "Timed out waiting for Instagram to process the video."
            ));
        }

        let publish_url = format!(
            "https://graph.facebook.com/v20.0/{}/media_publish",
            account_id.trim()
        );

        let pub_res = client
            .post(&publish_url)
            .header("Authorization", format!("Bearer {}", access_token.trim()))
            .json(&json!({
                "creation_id": container_id
            }))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send publish request: {e}"))?;

        if !pub_res.status().is_success() {
            let err = pub_res.text().await.unwrap_or_default();
            return Err(anyhow!("Instagram Reel publication failed: {err}"));
        }

        let pub_data: PublishResponse = pub_res
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse publish response: {e}"))?;

        let media_id = pub_data.id;

        let permalink_url = format!(
            "https://graph.facebook.com/v20.0/{}?fields=permalink&access_token={}",
            media_id,
            access_token.trim()
        );

        if let Ok(link_res) = client.get(&permalink_url).send().await {
            if let Ok(link_data) = link_res.json::<PermalinkResponse>().await {
                if let Some(link) = link_data.permalink {
                    return Ok(link);
                }
            }
        }

        Ok(format!("https://www.instagram.com/reel/{}/", media_id))
    }
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
