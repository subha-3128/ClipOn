use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::models::{CandidateDraft, NormalizedTranscript, SocialKit, TranscriptSegment};

#[derive(Debug, Deserialize)]
struct AnthropicMessage {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DeepseekMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct DeepseekChoice {
    message: DeepseekMessage,
}

#[derive(Debug, Deserialize)]
struct DeepseekResponse {
    choices: Vec<DeepseekChoice>,
}

pub async fn detect_candidates_with_deepseek(
    transcript: &NormalizedTranscript,
    api_key: &str,
    model_name: Option<&str>,
) -> Result<Vec<CandidateDraft>> {
    let segments = compact_segments(&transcript.segments);
    let prompt = format!(
        "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection, but extract AS MANY highly viral moments as possible. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
Clips should be 30-90 seconds long, completely self-contained, cut at clean boundaries, and deliver a massive payoff (a mind-blowing fact, hilarious joke, highly controversial opinion, or deep emotional insight). \
Return up to 25 candidates as JSON matching exactly this schema: \
{{\"candidates\":[{{\"start\":0.0,\"end\":0.0,\"score\":0.0,\"hook\":\"...\",\"rationale\":\"...\"}}]}}

Transcript:
{segments}"
    );

    let default_model = "deepseek-chat".to_string();
    let model = model_name
        .filter(|m| !m.trim().is_empty())
        .map(|m| m.trim().to_string())
        .or_else(|| {
            std::env::var("DEEPSEEK_MODEL")
                .ok()
                .filter(|m| !m.trim().is_empty())
        })
        .unwrap_or(default_model);

    let client = crate::http_client::build_api_client(60);
    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ],
        "temperature": 0.2,
        "response_format": {
            "type": "json_object"
        }
    });

    let response = crate::http_client::send_with_retry(
        &client,
        || {
            client
                .post("https://api.deepseek.com/chat/completions")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&payload)
        },
        3,
        800,
    )
    .await
    .context("calling DeepSeek API")?;

    let res_body: DeepseekResponse = response.json().await.context("parsing DeepSeek response")?;
    let text = res_body
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("DeepSeek response did not include choices content"))?;

    let min_duration = if transcript.duration < 60.0 {
        (transcript.duration * 0.5).max(5.0)
    } else {
        30.0
    };
    parse_candidate_json(&text, min_duration)
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

pub async fn detect_candidates_with_gemini(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let trimmed = api_key.trim().trim_matches('"').trim_matches('\'').trim();
    let clean_key = if trimmed.starts_with("Q.Ab8") {
        format!("A{trimmed}")
    } else {
        trimmed.to_string()
    };

    let segments = compact_segments(&transcript.segments);
    let prompt = format!(
        "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection, but extract AS MANY highly viral moments as possible. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
Clips should be 30-90 seconds long, completely self-contained, cut at clean boundaries, and deliver a massive payoff (a mind-blowing fact, hilarious joke, highly controversial opinion, or deep emotional insight). \
Return up to 25 candidates as JSON matching exactly this schema: \
{{\"candidates\":[{{\"start\":0.0,\"end\":0.0,\"score\":0.0,\"hook\":\"...\",\"rationale\":\"...\"}}]}}

Transcript:
{segments}"
    );

    let preferred_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3.5-flash".to_string());
    let mut models_to_try = vec![
        preferred_model,
        "gemini-3.5-flash".to_string(),
        "gemini-flash-latest".to_string(),
        "gemini-3.7-flash".to_string(),
        "gemini-flash-lite-latest".to_string(),
        "gemini-3.8-flash".to_string(),
    ];
    let mut seen = std::collections::HashSet::new();
    models_to_try.retain(|m| seen.insert(m.clone()));

    let mut last_error = String::new();

    for model in &models_to_try {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, clean_key
        );

        let response = match reqwest::Client::new()
            .post(&url)
            .header("x-goog-api-key", &clean_key)
            .json(&json!({
                "contents": [
                    {
                        "parts": [
                            {
                                "text": &prompt
                            }
                        ]
                    }
                ],
                "generationConfig": {
                    "responseMimeType": "application/json",
                    "temperature": 0.2
                }
            }))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("Network error calling Gemini ({model}): {e}");
                continue;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            last_error = format!("Gemini model {model} failed ({status}): {body}");
            if status.as_u16() == 503 || status.as_u16() == 429 || status.as_u16() == 404 {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                continue;
            }
            return Err(anyhow!("{last_error}"));
        }

        let res_body: GeminiResponse = match response.json().await {
            Ok(b) => b,
            Err(e) => {
                last_error = format!("Parsing Gemini ({model}) JSON response failed: {e}");
                continue;
            }
        };

        let text = match res_body
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .and_then(|p| p.text.clone())
        {
            Some(t) => t,
            None => {
                last_error = format!("Gemini ({model}) response did not include content text");
                continue;
            }
        };

        let min_duration = if transcript.duration < 60.0 {
            (transcript.duration * 0.5).max(5.0)
        } else {
            30.0
        };
        return parse_candidate_json(&text, min_duration);
    }

    Err(anyhow!(
        "All Gemini models failed. Last error: {last_error}"
    ))
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    content: String,
}

pub async fn detect_candidates_with_openai(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let segments = compact_segments(&transcript.segments);
    let prompt = format!(
        "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection, but extract AS MANY highly viral moments as possible. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
Clips should be 30-90 seconds long, completely self-contained, cut at clean boundaries, and deliver a massive payoff (a mind-blowing fact, hilarious joke, highly controversial opinion, or deep emotional insight). \
Return up to 25 candidates as JSON matching exactly this schema: \
{{\"candidates\":[{{\"start\":0.0,\"end\":0.0,\"score\":0.0,\"hook\":\"...\",\"rationale\":\"...\"}}]}}

Transcript:
{segments}"
    );

    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let client = crate::http_client::build_api_client(60);
    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ],
        "temperature": 0.2,
        "response_format": {
            "type": "json_object"
        }
    });

    let response = crate::http_client::send_with_retry(
        &client,
        || {
            client
                .post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&payload)
        },
        3,
        800,
    )
    .await
    .context("calling OpenAI API")?;

    let res_body: ChatCompletionResponse =
        response.json().await.context("parsing OpenAI response")?;
    let text = res_body
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("OpenAI response did not include choices content"))?;

    let min_duration = if transcript.duration < 60.0 {
        (transcript.duration * 0.5).max(5.0)
    } else {
        30.0
    };
    parse_candidate_json(&text, min_duration)
}

pub async fn detect_candidates_with_openrouter(
    transcript: &NormalizedTranscript,
    api_key: &str,
    model_name: Option<&str>,
) -> Result<Vec<CandidateDraft>> {
    let segments = compact_segments(&transcript.segments);
    let prompt = format!(
        "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection, but extract AS MANY highly viral moments as possible. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
Clips should be 30-90 seconds long, completely self-contained, cut at clean boundaries, and deliver a massive payoff (a mind-blowing fact, hilarious joke, highly controversial opinion, or deep emotional insight). \
Return up to 25 candidates as JSON matching exactly this schema: \
{{\"candidates\":[{{\"start\":0.0,\"end\":0.0,\"score\":0.0,\"hook\":\"...\",\"rationale\":\"...\"}}]}}

Transcript:
{segments}"
    );

    let default_model = "google/gemini-2.5-flash".to_string();
    let model = model_name
        .filter(|m| !m.trim().is_empty())
        .map(|m| m.trim().to_string())
        .or_else(|| {
            std::env::var("OPENROUTER_MODEL")
                .ok()
                .filter(|m| !m.trim().is_empty())
        })
        .unwrap_or(default_model);

    let client = crate::http_client::build_api_client(60);
    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ],
        "temperature": 0.2,
        "response_format": {
            "type": "json_object"
        }
    });

    let response = crate::http_client::send_with_retry(
        &client,
        || {
            client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&payload)
        },
        3,
        800,
    )
    .await
    .context("calling OpenRouter API")?;

    let res_body: ChatCompletionResponse = response
        .json()
        .await
        .context("parsing OpenRouter response")?;
    let text = res_body
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("OpenRouter response did not include choices content"))?;

    let min_duration = if transcript.duration < 60.0 {
        (transcript.duration * 0.5).max(5.0)
    } else {
        30.0
    };
    parse_candidate_json(&text, min_duration)
}

pub async fn detect_candidates_with_groq(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let segments = compact_segments(&transcript.segments);
    let prompt = format!(
        "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection, but extract AS MANY highly viral moments as possible. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
Clips should be 30-90 seconds long, completely self-contained, cut at clean boundaries, and deliver a massive payoff (a mind-blowing fact, hilarious joke, highly controversial opinion, or deep emotional insight). \
Return up to 25 candidates as JSON matching exactly this schema: \
{{\"candidates\":[{{\"start\":0.0,\"end\":0.0,\"score\":0.0,\"hook\":\"...\",\"rationale\":\"...\"}}]}}

Transcript:
{segments}"
    );

    let model =
        std::env::var("GROQ_MODEL").unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string());

    let client = crate::http_client::build_api_client(60);
    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ],
        "temperature": 0.2,
        "response_format": {
            "type": "json_object"
        }
    });

    let response = crate::http_client::send_with_retry(
        &client,
        || {
            client
                .post("https://api.groq.com/openai/v1/chat/completions")
                .header("Authorization", format!("Bearer {api_key}"))
                .json(&payload)
        },
        3,
        800,
    )
    .await
    .context("calling Groq API")?;

    let res_body: ChatCompletionResponse =
        response.json().await.context("parsing Groq response")?;
    let text = res_body
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("Groq response did not include choices content"))?;

    let min_duration = if transcript.duration < 60.0 {
        (transcript.duration * 0.5).max(5.0)
    } else {
        30.0
    };
    parse_candidate_json(&text, min_duration)
}

#[derive(Debug, Serialize)]
struct ClaudeMessage<'a> {
    role: &'a str,
    content: String,
}

pub async fn detect_candidates_with_claude(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let segments = compact_segments(&transcript.segments);
    let prompt = format!(
        "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection, but extract AS MANY highly viral moments as possible. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
Clips should be 30-90 seconds long, completely self-contained, cut at clean boundaries, and deliver a massive payoff (a mind-blowing fact, hilarious joke, highly controversial opinion, or deep emotional insight). \
Return up to 25 candidates as JSON matching exactly this schema: \
{{\"candidates\":[{{\"start\":0.0,\"end\":0.0,\"score\":0.0,\"hook\":\"...\",\"rationale\":\"...\"}}]}}

Transcript:
{segments}"
    );

    let model =
        std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet-latest".to_string());

    let client = crate::http_client::build_api_client(60);
    let payload = json!({
        "model": model,
        "max_tokens": 1800,
        "temperature": 0.2,
        "messages": [
            ClaudeMessage {
                role: "user",
                content: prompt,
            }
        ]
    });

    let response = crate::http_client::send_with_retry(
        &client,
        || {
            client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&payload)
        },
        3,
        800,
    )
    .await
    .context("calling Claude API")?;

    let message: AnthropicMessage = response.json().await.context("parsing Claude response")?;
    let text = message
        .content
        .into_iter()
        .find_map(|content| content.text)
        .ok_or_else(|| anyhow!("Claude response did not include text content"))?;

    let min_duration = if transcript.duration < 60.0 {
        (transcript.duration * 0.5).max(5.0)
    } else {
        30.0
    };
    parse_candidate_json(&text, min_duration)
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

pub async fn detect_candidates_with_local_llm(
    transcript: &NormalizedTranscript,
    model_name: &str,
) -> Result<Vec<CandidateDraft>> {
    let segments = compact_segments(&transcript.segments);

    let system_instructions = "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
CRITICAL: Each clip candidate MUST have a duration between 30 and 90 seconds (i.e. 'end' minus 'start' must be between 30.0 and 90.0). \
Do NOT return short clips of less than 30 seconds. Combine multiple adjacent sentences to build a meaningful segment of 30-90 seconds. \
Favor highly shareable content: concrete stories, strong opinions, emotional turns, surprising or counter-intuitive claims, clear payoffs, and high-energy/dramatic peaks. \
You MUST identify and return at least 3-10 candidates. Do not return an empty candidates list. \
Ensure the 'start' and 'end' values correspond to actual timestamps in the transcript. Do not output 0.0 for start and end times.";

    let user_content = format!("Transcript:\n{}", segments);

    let response = reqwest::Client::new()
        .post("http://localhost:11434/api/chat")
        .json(&json!({
            "model": model_name,
            "messages": [
                {
                    "role": "system",
                    "content": system_instructions,
                },
                {
                    "role": "user",
                    "content": user_content,
                }
            ],
            "stream": false,
            "options": {
                "temperature": 0.2
            },
            "format": {
                "type": "object",
                "properties": {
                    "candidates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "start": { "type": "number" },
                                "end": { "type": "number" },
                                "score": { "type": "number" },
                                "hook": { "type": "string" },
                                "rationale": { "type": "string" }
                            },
                            "required": ["start", "end", "score", "hook", "rationale"]
                        }
                    }
                },
                "required": ["candidates"]
            }
        }))
        .send()
        .await
        .context("calling local Ollama")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("Local Ollama request failed ({status}): {body}"));
    }

    let res_body: OllamaResponse = response
        .json()
        .await
        .context("parsing local Ollama response")?;
    let min_duration = if transcript.duration < 60.0 {
        (transcript.duration * 0.5).max(5.0)
    } else {
        30.0
    };
    parse_candidate_json(&res_body.message.content, min_duration)
}

fn compact_segments(segments: &[TranscriptSegment]) -> String {
    // If transcript is exceedingly long (> 2 hours, > 1200 segments), sample across the timeline
    // to prevent LLM prompt token exhaustion while preserving full temporal coverage.
    let sampled_segments: Vec<&TranscriptSegment> = if segments.len() > 1200 {
        let step = (segments.len() as f64 / 1200.0).ceil() as usize;
        segments.iter().step_by(step.max(1)).collect()
    } else {
        segments.iter().collect()
    };

    sampled_segments
        .into_iter()
        .map(|segment| {
            let speaker = segment.speaker.as_deref().unwrap_or("Speaker");
            format!(
                "[{:.2}-{:.2}] {}: {}",
                segment.start, segment.end, speaker, segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_candidate_json(text: &str, min_duration: f64) -> Result<Vec<CandidateDraft>> {
    let trimmed = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let val: serde_json::Value = serde_json::from_str(trimmed).context("parsing candidate JSON")?;

    let candidates_arr = if val.is_array() {
        val.as_array().cloned()
    } else if val.is_object() {
        let mut found_arr = None;
        for key in &[
            "candidates",
            "Candidates",
            "moments",
            "clips",
            "segments",
            "results",
        ] {
            if let Some(arr) = val.get(*key).and_then(|v| v.as_array()) {
                found_arr = Some(arr.clone());
                break;
            }
        }
        if found_arr.is_none() {
            if let Some(obj) = val.as_object() {
                for (_key, value) in obj {
                    if let Some(arr) = value.as_array() {
                        found_arr = Some(arr.clone());
                        break;
                    }
                }
            }
        }
        if found_arr.is_some() {
            found_arr
        } else if val.get("start").is_some() && val.get("end").is_some() {
            Some(vec![val.clone()])
        } else {
            None
        }
    } else {
        None
    };

    let concrete_arr = candidates_arr.ok_or_else(|| {
        anyhow!(
            "Ollama output does not contain a candidates array. Raw output: {}",
            trimmed
        )
    })?;

    let mut drafts = Vec::new();
    for item in &concrete_arr {
        let start = match item.get("start") {
            Some(v) => {
                if let Some(f) = v.as_f64() {
                    f
                } else if let Some(s) = v.as_str() {
                    s.parse::<f64>().unwrap_or(0.0)
                } else if let Some(i) = v.as_i64() {
                    i as f64
                } else {
                    0.0
                }
            }
            None => 0.0,
        };

        let end = match item.get("end") {
            Some(v) => {
                if let Some(f) = v.as_f64() {
                    f
                } else if let Some(s) = v.as_str() {
                    s.parse::<f64>().unwrap_or(0.0)
                } else if let Some(i) = v.as_i64() {
                    i as f64
                } else {
                    0.0
                }
            }
            None => 0.0,
        };

        let mut score = match item.get("score") {
            Some(v) => {
                if let Some(f) = v.as_f64() {
                    f
                } else if let Some(s) = v.as_str() {
                    s.parse::<f64>().unwrap_or(0.8)
                } else if let Some(i) = v.as_i64() {
                    i as f64
                } else {
                    0.8
                }
            }
            None => 0.8,
        };

        if score > 1.0 && score <= 10.0 {
            score /= 10.0;
        } else if score > 10.0 && score <= 100.0 {
            score /= 100.0;
        } else if score > 100.0 {
            score = 1.0;
        } else if score < 0.0 {
            score = 0.0;
        }

        let hook = item
            .get("hook")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let rationale = item
            .get("rationale")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        drafts.push(CandidateDraft {
            start,
            end,
            score,
            hook,
            rationale,
        });
    }

    let mut candidates = drafts
        .clone()
        .into_iter()
        .filter(|candidate| {
            (candidate.end - candidate.start) >= min_duration && !candidate.hook.trim().is_empty()
        })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        candidates = drafts
            .into_iter()
            .filter(|candidate| {
                (candidate.end - candidate.start) >= 5.0 && !candidate.hook.trim().is_empty()
            })
            .collect::<Vec<_>>();
    }

    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    candidates.truncate(10);
    Ok(candidates)
}

#[derive(Debug, Deserialize)]
struct SocialKitJson {
    titles: Option<Vec<String>>,
    description: Option<String>,
    hashtags: Option<Vec<String>>,
    call_to_action: Option<String>,
}

pub fn generate_heuristic_social_kit(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
) -> SocialKit {
    let clean_hook = hook.trim().trim_end_matches('.').trim_end_matches('!');
    let titles = vec![
        format!("🔥 {clean_hook}"),
        format!("The Truth About: {clean_hook} 🤯"),
        format!("Watch This Before It's Too Late: {clean_hook}"),
    ];

    let snippet = if transcript_text.len() > 140 {
        format!("{}...", &transcript_text[..140].trim())
    } else {
        transcript_text.trim().to_string()
    };

    let description = if snippet.is_empty() {
        format!("{clean_hook}. What do you think about this? Let me know below! 👇")
    } else {
        format!(
            "{clean_hook} — {snippet}

Save this for later! 📌"
        )
    };

    let hashtags = vec![
        "#shorts".to_string(),
        "#viral".to_string(),
        "#trending".to_string(),
        "#reels".to_string(),
        "#tiktok".to_string(),
        "#growth".to_string(),
        "#mindset".to_string(),
    ];

    SocialKit {
        candidate_id: candidate_id.to_string(),
        titles,
        description,
        hashtags,
        call_to_action: "Drop a 🔥 in the comments if you agree! 👇".to_string(),
    }
}

pub async fn generate_social_kit_with_gemini(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    api_key: &str,
) -> Result<SocialKit> {
    let trimmed = api_key.trim().trim_matches('"').trim_matches('\'').trim();
    let clean_key = if trimmed.starts_with("Q.Ab8") {
        format!("A{trimmed}")
    } else {
        trimmed.to_string()
    };

    let prompt = format!(
        "You are an elite viral social media strategist for YouTube Shorts, TikTok, and Instagram Reels. \
Given this clip's hook and spoken transcript, generate a complete high-converting social media posting kit. \
Include: \
1. Exactly 3 compelling, viral titles (high CTR, curiosity-gap, or emotional payoff). \
2. A concise 1-2 sentence caption/description suitable for Reels and Shorts. \
3. 5-8 relevant trending hashtags. \
4. A punchy Call to Action (CTA) question to drive comments. \
\
Hook: {hook} \
Transcript: {transcript_text} \
\
Return JSON matching exactly: \
{{\"titles\": [\"title 1\", \"title 2\", \"title 3\"], \"description\": \"...\", \"hashtags\": [\"#tag1\", \"#tag2\"], \"call_to_action\": \"...\"}}"
    );

    let preferred_model =
        std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-3.5-flash".to_string());
    let models_to_try = vec![
        preferred_model,
        "gemini-3.5-flash".to_string(),
        "gemini-flash-latest".to_string(),
        "gemini-3.7-flash".to_string(),
        "gemini-3.8-flash".to_string(),
    ];

    for model in &models_to_try {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, clean_key
        );

        let response = reqwest::Client::new()
            .post(&url)
            .header("x-goog-api-key", &clean_key)
            .json(&json!({
                "contents": [
                    {
                        "parts": [
                            {
                                "text": &prompt
                            }
                        ]
                    }
                ],
                "generationConfig": {
                    "responseMimeType": "application/json",
                    "temperature": 0.3
                }
            }))
            .send()
            .await;

        if let Ok(resp) = response {
            if resp.status().is_success() {
                if let Ok(body) = resp.json::<GeminiResponse>().await {
                    if let Some(content) = body
                        .candidates
                        .first()
                        .and_then(|c| c.content.parts.first())
                        .and_then(|p| p.text.as_ref())
                    {
                        if let Ok(kit) = serde_json::from_str::<SocialKitJson>(content) {
                            return Ok(SocialKit {
                                candidate_id: candidate_id.to_string(),
                                titles: kit.titles.unwrap_or_else(|| vec![hook.to_string()]),
                                description: kit
                                    .description
                                    .unwrap_or_else(|| format!("{hook} - Watch till the end!")),
                                hashtags: kit.hashtags.unwrap_or_else(|| {
                                    vec![
                                        "#shorts".to_string(),
                                        "#viral".to_string(),
                                        "#fyp".to_string(),
                                    ]
                                }),
                                call_to_action: kit.call_to_action.unwrap_or_else(|| {
                                    "What do you think? Let me know below! 👇".to_string()
                                }),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(generate_heuristic_social_kit(
        candidate_id,
        hook,
        transcript_text,
    ))
}

pub async fn generate_social_kit_with_deepseek(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    api_key: &str,
) -> Result<SocialKit> {
    let prompt = format!(
        "You are an elite viral social media manager for YouTube Shorts, Reels, and TikTok. \
Given this clip's hook and spoken transcript, generate a complete high-converting social media posting kit. \
Include: \
1. Exactly 3 compelling, viral titles (high CTR, curiosity-gap, or emotional payoff). \
2. A concise 1-2 sentence caption/description suitable for Reels and Shorts. \
3. 5-8 relevant trending hashtags. \
4. A punchy Call to Action (CTA) question to drive comments. \
\
Hook: {hook} \
Transcript: {transcript_text} \
\
Return JSON matching exactly: \
{{\"titles\": [\"title 1\", \"title 2\", \"title 3\"], \"description\": \"...\", \"hashtags\": [\"#tag1\", \"#tag2\"], \"call_to_action\": \"...\"}}"
    );

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.deepseek.com/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&json!({
            "model": "deepseek-chat",
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.3,
            "response_format": {"type": "json_object"}
        }))
        .send()
        .await
        .context("calling DeepSeek")?;

    if !response.status().is_success() {
        return Err(anyhow!("DeepSeek request failed"));
    }

    let res_body: DeepseekResponse = response.json().await?;
    let text = res_body
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("No content"))?;

    let parsed: SocialKitJson = serde_json::from_str(&text)?;
    Ok(SocialKit {
        candidate_id: candidate_id.to_string(),
        titles: parsed.titles.unwrap_or_else(|| vec![hook.to_string()]),
        description: parsed
            .description
            .unwrap_or_else(|| format!("{hook} - Watch till the end!")),
        hashtags: parsed.hashtags.unwrap_or_else(|| {
            vec![
                "#shorts".to_string(),
                "#viral".to_string(),
                "#reels".to_string(),
            ]
        }),
        call_to_action: parsed
            .call_to_action
            .unwrap_or_else(|| "What are your thoughts? Drop a comment! 👇".to_string()),
    })
}

pub async fn generate_social_kit(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
) -> SocialKit {
    if let Ok(Some(key)) = crate::credentials::get(crate::credentials::GEMINI) {
        if !key.trim().is_empty() {
            if let Ok(kit) =
                generate_social_kit_with_gemini(candidate_id, hook, transcript_text, &key).await
            {
                return kit;
            }
        }
    }

    if let Ok(Some(key)) = crate::credentials::get(crate::credentials::DEEPSEEK) {
        if !key.trim().is_empty() {
            if let Ok(kit) =
                generate_social_kit_with_deepseek(candidate_id, hook, transcript_text, &key).await
            {
                return kit;
            }
        }
    }

    generate_heuristic_social_kit(candidate_id, hook, transcript_text)
}
