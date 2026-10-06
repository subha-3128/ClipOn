use anyhow::{anyhow, Result};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use std::time::Duration;
use tokio::time::sleep;

pub fn build_api_client(timeout_secs: u64) -> Client {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| Client::new())
}

pub async fn send_with_retry<F>(
    _client: &Client,
    build_request: F,
    max_retries: usize,
    base_backoff_ms: u64,
) -> Result<Response>
where
    F: Fn() -> RequestBuilder,
{
    let mut attempt = 0;
    loop {
        let req = build_request();
        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp);
                }

                // Non-retryable authentication failure
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(anyhow!("Authentication failed ({status}): {body}"));
                }

                // Non-retryable client bad request
                if status == StatusCode::BAD_REQUEST || status == StatusCode::NOT_FOUND {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(anyhow!("Invalid request ({status}): {body}"));
                }

                // Check for Retry-After header
                let retry_after_secs = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());

                // Retryable: 429 Too Many Requests or 5xx Server Errors
                let is_retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
                if is_retryable && attempt < max_retries {
                    attempt += 1;
                    let backoff = if let Some(secs) = retry_after_secs {
                        Duration::from_secs(secs.min(5))
                    } else {
                        Duration::from_millis(base_backoff_ms * (1 << (attempt - 1)))
                    };
                    sleep(backoff).await;
                    continue;
                }

                let body = resp.text().await.unwrap_or_default();
                if status == StatusCode::TOO_MANY_REQUESTS {
                    let hint = retry_after_secs
                        .map(|s| format!("Rate limit reached. Retry after {}s.", s))
                        .unwrap_or_else(|| "Rate limit reached. Please wait before retrying.".to_string());
                    return Err(anyhow!("{hint} ({body})"));
                }
                if status == StatusCode::SERVICE_UNAVAILABLE {
                    return Err(anyhow!("Provider is currently overloaded (503). Please try again shortly. ({body})"));
                }
                return Err(anyhow!("API request failed with status {status}: {body}"));
            }
            Err(e) => {
                let is_retryable = e.is_timeout() || e.is_connect();
                if is_retryable && attempt < max_retries {
                    attempt += 1;
                    let backoff = Duration::from_millis(base_backoff_ms * (1 << (attempt - 1)));
                    sleep(backoff).await;
                    continue;
                }
                return Err(anyhow!("Network request error: {e}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_api_client() {
        let client = build_api_client(30);
        // Valid client constructed
        assert!(format!("{:?}", client).contains("Client"));
    }
}
