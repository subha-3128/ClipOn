use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::models::{CandidateDraft, NormalizedTranscript, SocialKit, TranscriptSegment};

#[allow(async_fn_in_trait)]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn complete(&self, prompt: &str, json_mode: bool, temperature: f64) -> Result<String>;
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionChoice {
    pub message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
pub struct ChatCompletionMessage {
    pub content: String,
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

#[derive(Debug, Deserialize)]
struct AnthropicMessage {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

pub fn resolve_gemini_models() -> Vec<String> {
    let mut models = Vec::new();
    if let Ok(m) = std::env::var("GEMINI_MODEL") {
        let trimmed = m.trim();
        if !trimmed.is_empty() {
            models.push(trimmed.to_string());
        }
    }
    for default_m in ["gemini-2.0-flash", "gemini-1.5-flash", "gemini-1.5-pro"] {
        if !models.iter().any(|m| m == default_m) {
            models.push(default_m.to_string());
        }
    }
    models
}

pub struct GeminiProvider {
    pub api_key: String,
    pub models: Vec<String>,
}

impl GeminiProvider {
    pub fn new(api_key: &str) -> Self {
        let trimmed = api_key.trim().trim_matches('"').trim_matches('\'').trim();
        let clean_key = if trimmed.starts_with("Q.Ab8") {
            format!("A{trimmed}")
        } else {
            trimmed.to_string()
        };
        Self {
            api_key: clean_key,
            models: resolve_gemini_models(),
        }
    }
}

impl LlmProvider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    async fn complete(&self, prompt: &str, _json_mode: bool, temperature: f64) -> Result<String> {
        let mut last_error = String::new();
        for model in &self.models {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                model
            );

            let response = match reqwest::Client::new()
                .post(&url)
                .header("x-goog-api-key", &self.api_key)
                .json(&json!({
                    "contents": [
                        {
                            "parts": [
                                {
                                    "text": prompt
                                }
                            ]
                        }
                    ],
                    "generationConfig": {
                        "responseMimeType": "application/json",
                        "temperature": temperature
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

            return Ok(text);
        }

        Err(anyhow!("All Gemini models failed. Last error: {last_error}"))
    }
}

pub async fn call_openai_compatible_api(
    provider_name: &str,
    endpoint: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    json_mode: bool,
    temperature: f64,
    extra_headers: &[(&str, &str)],
) -> Result<String> {
    let client = crate::http_client::build_api_client(60);
    let mut payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": prompt,
            }
        ],
        "temperature": temperature,
    });
    if json_mode {
        payload["response_format"] = json!({
            "type": "json_object"
        });
    }

    let response = crate::http_client::send_with_retry(
        &client,
        || {
            let mut req = client
                .post(endpoint)
                .header("Authorization", format!("Bearer {api_key}"));
            for (k, v) in extra_headers {
                req = req.header(*k, *v);
            }
            req.json(&payload)
        },
        3,
        800,
    )
    .await
    .context(format!("calling {provider_name} API"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!("{provider_name} request failed ({status}): {body}"));
    }

    let res_body: ChatCompletionResponse = response
        .json()
        .await
        .context(format!("parsing {provider_name} response"))?;
    res_body
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("{provider_name} response did not include choices content"))
}

pub struct OpenAiProvider {
    pub api_key: String,
    pub model: String,
}

impl OpenAiProvider {
    pub fn new(api_key: &str, model_override: Option<&str>) -> Self {
        let model = model_override
            .filter(|m| !m.trim().is_empty())
            .map(|m| m.trim().to_string())
            .or_else(|| std::env::var("OPENAI_MODEL").ok().filter(|m| !m.trim().is_empty()))
            .unwrap_or_else(|| "gpt-4o-mini".to_string());
        Self {
            api_key: api_key.trim().to_string(),
            model,
        }
    }
}

impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn complete(&self, prompt: &str, json_mode: bool, temperature: f64) -> Result<String> {
        call_openai_compatible_api(
            "OpenAI",
            "https://api.openai.com/v1/chat/completions",
            &self.api_key,
            &self.model,
            prompt,
            json_mode,
            temperature,
            &[],
        )
        .await
    }
}

pub struct DeepSeekProvider {
    pub api_key: String,
    pub model: String,
}

impl DeepSeekProvider {
    pub fn new(api_key: &str, model_override: Option<&str>) -> Self {
        let model = model_override
            .filter(|m| !m.trim().is_empty())
            .map(|m| m.trim().to_string())
            .or_else(|| {
                std::env::var("DEEPSEEK_MODEL")
                    .ok()
                    .filter(|m| !m.trim().is_empty())
            })
            .unwrap_or_else(|| "deepseek-chat".to_string());
        Self {
            api_key: api_key.trim().to_string(),
            model,
        }
    }
}

impl LlmProvider for DeepSeekProvider {
    fn name(&self) -> &str {
        "deepseek"
    }

    async fn complete(&self, prompt: &str, json_mode: bool, temperature: f64) -> Result<String> {
        call_openai_compatible_api(
            "DeepSeek",
            "https://api.deepseek.com/chat/completions",
            &self.api_key,
            &self.model,
            prompt,
            json_mode,
            temperature,
            &[],
        )
        .await
    }
}

pub struct GroqProvider {
    pub api_key: String,
    pub model: String,
}

impl GroqProvider {
    pub fn new(api_key: &str, model_override: Option<&str>) -> Self {
        let model = model_override
            .filter(|m| !m.trim().is_empty())
            .map(|m| m.trim().to_string())
            .or_else(|| std::env::var("GROQ_MODEL").ok().filter(|m| !m.trim().is_empty()))
            .unwrap_or_else(|| "llama-3.3-70b-versatile".to_string());
        Self {
            api_key: api_key.trim().to_string(),
            model,
        }
    }
}

impl LlmProvider for GroqProvider {
    fn name(&self) -> &str {
        "groq"
    }

    async fn complete(&self, prompt: &str, json_mode: bool, temperature: f64) -> Result<String> {
        call_openai_compatible_api(
            "Groq",
            "https://api.groq.com/openai/v1/chat/completions",
            &self.api_key,
            &self.model,
            prompt,
            json_mode,
            temperature,
            &[],
        )
        .await
    }
}

pub struct OpenRouterProvider {
    pub api_key: String,
    pub model: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: &str, model_override: Option<&str>) -> Self {
        let model = model_override
            .filter(|m| !m.trim().is_empty())
            .map(|m| m.trim().to_string())
            .or_else(|| {
                std::env::var("OPENROUTER_MODEL")
                    .ok()
                    .filter(|m| !m.trim().is_empty())
            })
            .unwrap_or_else(|| "google/gemini-2.5-flash".to_string());
        Self {
            api_key: api_key.trim().to_string(),
            model,
        }
    }
}

impl LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    async fn complete(&self, prompt: &str, json_mode: bool, temperature: f64) -> Result<String> {
        call_openai_compatible_api(
            "OpenRouter",
            "https://openrouter.ai/api/v1/chat/completions",
            &self.api_key,
            &self.model,
            prompt,
            json_mode,
            temperature,
            &[],
        )
        .await
    }
}

pub struct ClaudeProvider {
    pub api_key: String,
    pub model: String,
}

impl ClaudeProvider {
    pub fn new(api_key: &str, model_override: Option<&str>) -> Self {
        let model = model_override
            .filter(|m| !m.trim().is_empty())
            .map(|m| m.trim().to_string())
            .or_else(|| {
                std::env::var("ANTHROPIC_MODEL")
                    .ok()
                    .filter(|m| !m.trim().is_empty())
            })
            .unwrap_or_else(|| "claude-3-5-sonnet-latest".to_string());
        Self {
            api_key: api_key.trim().to_string(),
            model,
        }
    }
}

impl LlmProvider for ClaudeProvider {
    fn name(&self) -> &str {
        "claude"
    }

    async fn complete(&self, prompt: &str, _json_mode: bool, temperature: f64) -> Result<String> {
        let client = crate::http_client::build_api_client(60);
        let payload = json!({
            "model": self.model,
            "max_tokens": 1800,
            "temperature": temperature,
            "messages": [
                {
                    "role": "user",
                    "content": prompt,
                }
            ]
        });

        let response = crate::http_client::send_with_retry(
            &client,
            || {
                client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&payload)
            },
            3,
            800,
        )
        .await
        .context("calling Claude API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Claude request failed ({status}): {body}"));
        }

        let message: AnthropicMessage =
            response.json().await.context("parsing Claude response")?;
        message
            .content
            .into_iter()
            .find_map(|content| content.text)
            .ok_or_else(|| anyhow!("Claude response did not include text content"))
    }
}

pub struct LocalLlmProvider {
    pub model: String,
    pub host: String,
}

impl LocalLlmProvider {
    pub fn new(model_name: Option<&str>) -> Self {
        let model = model_name
            .filter(|m| !m.trim().is_empty())
            .map(|m| m.trim().to_string())
            .or_else(|| std::env::var("OLLAMA_MODEL").ok().filter(|m| !m.trim().is_empty()))
            .unwrap_or_else(|| "llama3.2".to_string());
        let host = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self { model, host }
    }
}

impl LlmProvider for LocalLlmProvider {
    fn name(&self) -> &str {
        "local"
    }

    async fn complete(&self, prompt: &str, _json_mode: bool, temperature: f64) -> Result<String> {
        let client = crate::http_client::build_api_client(90);
        let url = format!("{}/api/chat", self.host.trim_end_matches('/'));
        let response = client
            .post(&url)
            .json(&json!({
                "model": self.model,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt,
                    }
                ],
                "stream": false,
                "format": "json",
                "options": {
                    "temperature": temperature
                }
            }))
            .send()
            .await
            .context("calling local Ollama chat API")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Local Ollama request failed ({status}): {body}"));
        }

        let res_body: OllamaResponse = response
            .json()
            .await
            .context("parsing local Ollama response")?;
        Ok(res_body.message.content)
    }
}

pub fn build_candidate_prompt(segments: &str) -> String {
    format!(
        "You are an elite, world-class social media strategist with a track record of generating viral multi-million-view Shorts, TikToks, and Reels. \
Your sole objective is to identify the ABSOLUTE BEST, most highly-engaging, and trend-setting short-form clip candidates from the provided transcript. \
Do NOT pick random or mediocre segments. Be ruthless in your selection, but extract AS MANY highly viral moments as possible. \
Every candidate must have an insanely strong, curiosity-inducing hook in the first 3 seconds to stop the scroll. \
Clips should be 30-90 seconds long, completely self-contained, cut at clean boundaries, and deliver a massive payoff (a mind-blowing fact, hilarious joke, highly controversial opinion, or deep emotional insight). \
Return up to 25 candidates as JSON matching exactly this schema: \
{{\"candidates\":[{{\"start\":0.0,\"end\":0.0,\"score\":0.0,\"hook\":\"...\",\"rationale\":\"...\"}}]}}

Transcript:
{segments}"
    )
}

pub async fn detect_chunk_with_provider<P: LlmProvider + ?Sized>(
    provider: &P,
    chunk: &NormalizedTranscript,
    full_duration: f64,
) -> Result<Vec<CandidateDraft>> {
    let segments = compact_segments(&chunk.segments);
    let prompt = build_candidate_prompt(&segments);
    let text = provider.complete(&prompt, true, 0.2).await?;
    let min_duration = if full_duration < 60.0 {
        (full_duration * 0.5).max(5.0)
    } else {
        20.0
    };
    parse_candidate_json(&text, min_duration, full_duration)
}

pub async fn detect_candidates_with_provider<P: LlmProvider + ?Sized>(
    provider: &P,
    transcript: &NormalizedTranscript,
) -> Result<Vec<CandidateDraft>> {
    let chunks = chunk_transcript(transcript, 1200.0, 90.0);
    let mut all_drafts = Vec::new();
    for chunk in &chunks {
        let drafts = detect_chunk_with_provider(provider, chunk, transcript.duration).await?;
        all_drafts.extend(drafts);
    }
    Ok(deduplicate_and_rank_candidates(all_drafts, 15))
}

pub async fn detect_candidates_with_gemini(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let provider = GeminiProvider::new(api_key);
    detect_candidates_with_provider(&provider, transcript).await
}

pub async fn detect_candidates_with_deepseek(
    transcript: &NormalizedTranscript,
    api_key: &str,
    model_name: Option<&str>,
) -> Result<Vec<CandidateDraft>> {
    let provider = DeepSeekProvider::new(api_key, model_name);
    detect_candidates_with_provider(&provider, transcript).await
}

pub async fn detect_candidates_with_openai(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let provider = OpenAiProvider::new(api_key, None);
    detect_candidates_with_provider(&provider, transcript).await
}

pub async fn detect_candidates_with_openrouter(
    transcript: &NormalizedTranscript,
    api_key: &str,
    model_name: Option<&str>,
) -> Result<Vec<CandidateDraft>> {
    let provider = OpenRouterProvider::new(api_key, model_name);
    detect_candidates_with_provider(&provider, transcript).await
}

pub async fn detect_candidates_with_groq(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let provider = GroqProvider::new(api_key, None);
    detect_candidates_with_provider(&provider, transcript).await
}

pub async fn detect_candidates_with_claude(
    transcript: &NormalizedTranscript,
    api_key: &str,
) -> Result<Vec<CandidateDraft>> {
    let provider = ClaudeProvider::new(api_key, None);
    detect_candidates_with_provider(&provider, transcript).await
}

pub async fn detect_candidates_with_local_llm(
    transcript: &NormalizedTranscript,
    model_name: &str,
) -> Result<Vec<CandidateDraft>> {
    let provider = LocalLlmProvider::new(Some(model_name));
    detect_candidates_with_provider(&provider, transcript).await
}

fn compact_segments(segments: &[TranscriptSegment]) -> String {
    // Preserve full conversation across segments without dropping any speech lines
    segments
        .iter()
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

pub fn chunk_transcript(
    transcript: &NormalizedTranscript,
    chunk_duration_sec: f64,
    overlap_sec: f64,
) -> Vec<NormalizedTranscript> {
    if transcript.duration <= chunk_duration_sec && transcript.segments.len() <= 600 {
        return vec![transcript.clone()];
    }

    let mut chunks = Vec::new();
    let step = (chunk_duration_sec - overlap_sec).max(60.0);
    let mut window_start = 0.0;

    while window_start < transcript.duration {
        let window_end = (window_start + chunk_duration_sec).min(transcript.duration);
        let chunk_segments: Vec<TranscriptSegment> = transcript
            .segments
            .iter()
            .filter(|s| s.start < window_end && s.end > window_start)
            .cloned()
            .collect();

        if !chunk_segments.is_empty() {
            chunks.push(NormalizedTranscript {
                language: transcript.language.clone(),
                duration: transcript.duration,
                speakers: transcript.speakers.clone(),
                words: Vec::new(),
                segments: chunk_segments,
            });
        }

        if window_end >= transcript.duration {
            break;
        }
        window_start += step;
    }

    if chunks.is_empty() {
        vec![transcript.clone()]
    } else {
        chunks
    }
}

pub fn deduplicate_and_rank_candidates(
    mut candidates: Vec<CandidateDraft>,
    max_count: usize,
) -> Vec<CandidateDraft> {
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));

    let mut deduplicated: Vec<CandidateDraft> = Vec::new();
    for cand in candidates {
        let is_dup = deduplicated.iter().any(|existing| {
            let intersection =
                (cand.end.min(existing.end) - cand.start.max(existing.start)).max(0.0);
            let dur_a = cand.end - cand.start;
            let dur_b = existing.end - existing.start;
            let min_dur = dur_a.min(dur_b);

            (min_dur > 0.0 && intersection / min_dur > 0.4)
                || (cand.start - existing.start).abs() < 5.0
        });

        if !is_dup {
            deduplicated.push(cand);
        }
    }

    deduplicated.sort_by(|a, b| b.score.total_cmp(&a.score));
    deduplicated.truncate(max_count);
    deduplicated
}

fn parse_number(val: Option<&serde_json::Value>) -> Option<f64> {
    let v = val?;
    let n = if let Some(f) = v.as_f64() {
        f
    } else if let Some(s) = v.as_str() {
        s.trim().parse::<f64>().ok()?
    } else if let Some(i) = v.as_i64() {
        i as f64
    } else {
        return None;
    };
    if n.is_finite() {
        Some(n)
    } else {
        None
    }
}

pub fn parse_candidate_json(
    text: &str,
    min_duration: f64,
    max_timeline_duration: f64,
) -> Result<Vec<CandidateDraft>> {
    let trimmed = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let val: serde_json::Value =
        serde_json::from_str(trimmed).context("parsing candidate JSON")?;

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
            "Model output does not contain a candidates array. Raw output: {}",
            trimmed
        )
    })?;

    let mut drafts = Vec::new();
    for item in &concrete_arr {
        let start = match parse_number(item.get("start")) {
            Some(s) => s,
            None => continue,
        };

        let end = match parse_number(item.get("end")) {
            Some(e) => e,
            None => continue,
        };

        if start < 0.0 || end <= start {
            continue;
        }

        let duration = end - start;
        if duration > 300.0 {
            continue;
        }

        if max_timeline_duration > 0.0 {
            if start > max_timeline_duration + 1.0 {
                continue;
            }
            if end > max_timeline_duration + 5.0 {
                continue;
            }
        }

        let raw_score = parse_number(item.get("score")).unwrap_or(0.7);
        let mut score = if raw_score > 1.0 && raw_score <= 10.0 {
            raw_score / 10.0
        } else if raw_score > 10.0 && raw_score <= 100.0 {
            raw_score / 100.0
        } else {
            raw_score
        };
        score = score.clamp(0.0, 1.0);

        let hook = item
            .get("hook")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        if hook.is_empty() {
            continue;
        }

        let rationale = item
            .get("rationale")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        drafts.push(CandidateDraft {
            start,
            end: if max_timeline_duration > 0.0 {
                end.min(max_timeline_duration)
            } else {
                end
            },
            score,
            hook,
            rationale,
        });
    }

    let mut candidates = drafts
        .iter()
        .filter(|candidate| (candidate.end - candidate.start) >= min_duration)
        .cloned()
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        candidates = drafts
            .iter()
            .filter(|candidate| (candidate.end - candidate.start) >= 5.0)
            .cloned()
            .collect::<Vec<_>>();
    }

    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
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

    let snippet = if transcript_text.chars().count() > 140 {
        let truncated: String = transcript_text.chars().take(140).collect();
        format!("{}...", truncated.trim())
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

pub fn build_social_kit_prompt(hook: &str, transcript_text: &str) -> String {
    format!(
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
    )
}

fn kit_from_json_text(text: &str, candidate_id: &str, hook: &str) -> Result<SocialKit> {
    let clean = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let parsed: SocialKitJson = serde_json::from_str(clean)?;
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

pub async fn generate_social_kit_with_provider<P: LlmProvider + ?Sized>(
    provider: &P,
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
) -> Result<SocialKit> {
    let prompt = build_social_kit_prompt(hook, transcript_text);
    match provider.complete(&prompt, true, 0.3).await {
        Ok(text) => kit_from_json_text(&text, candidate_id, hook),
        Err(_) => Ok(generate_heuristic_social_kit(
            candidate_id,
            hook,
            transcript_text,
        )),
    }
}

pub async fn generate_social_kit_with_gemini(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    api_key: &str,
) -> Result<SocialKit> {
    let provider = GeminiProvider::new(api_key);
    generate_social_kit_with_provider(&provider, candidate_id, hook, transcript_text).await
}

pub async fn generate_social_kit_with_deepseek(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    api_key: &str,
) -> Result<SocialKit> {
    let provider = DeepSeekProvider::new(api_key, None);
    generate_social_kit_with_provider(&provider, candidate_id, hook, transcript_text).await
}

pub async fn generate_social_kit_openai_compat(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    endpoint: &str,
    api_key: &str,
    model: &str,
    extra_headers: &[(&str, &str)],
) -> Result<SocialKit> {
    struct CompatProvider<'a> {
        endpoint: &'a str,
        api_key: &'a str,
        model: &'a str,
        extra_headers: &'a [(&'a str, &'a str)],
    }
    impl<'a> LlmProvider for CompatProvider<'a> {
        fn name(&self) -> &str {
            "openai-compat"
        }
        async fn complete(&self, prompt: &str, json_mode: bool, temperature: f64) -> Result<String> {
            call_openai_compatible_api(
                "OpenAI-Compat",
                self.endpoint,
                self.api_key,
                self.model,
                prompt,
                json_mode,
                temperature,
                self.extra_headers,
            )
            .await
        }
    }
    let provider = CompatProvider {
        endpoint,
        api_key,
        model,
        extra_headers,
    };
    generate_social_kit_with_provider(&provider, candidate_id, hook, transcript_text).await
}

pub async fn generate_social_kit_with_claude(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    api_key: &str,
) -> Result<SocialKit> {
    let provider = ClaudeProvider::new(api_key, None);
    generate_social_kit_with_provider(&provider, candidate_id, hook, transcript_text).await
}

pub async fn generate_social_kit_with_local(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    model: &str,
) -> Result<SocialKit> {
    let provider = LocalLlmProvider::new(Some(model));
    generate_social_kit_with_provider(&provider, candidate_id, hook, transcript_text).await
}

pub async fn generate_social_kit(
    candidate_id: &str,
    hook: &str,
    transcript_text: &str,
    provider: Option<&str>,
    model_name: Option<&str>,
) -> SocialKit {
    let env_provider = std::env::var("LLM_PROVIDER").ok();
    let active_provider = provider
        .or(env_provider.as_deref())
        .unwrap_or("deepseek")
        .to_lowercase();

    let res = match active_provider.as_str() {
        "claude" => {
            if let Ok(Some(key)) = crate::credentials::get(crate::credentials::ANTHROPIC) {
                if !key.trim().is_empty() {
                    generate_social_kit_with_claude(candidate_id, hook, transcript_text, &key).await.ok()
                } else { None }
            } else { None }
        }
        "gemini" => {
            if let Ok(Some(key)) = crate::credentials::get(crate::credentials::GEMINI) {
                if !key.trim().is_empty() {
                    generate_social_kit_with_gemini(candidate_id, hook, transcript_text, &key).await.ok()
                } else { None }
            } else { None }
        }
        "openai" => {
            if let Ok(Some(key)) = crate::credentials::get(crate::credentials::OPENAI) {
                if !key.trim().is_empty() {
                    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
                    generate_social_kit_openai_compat(
                        candidate_id,
                        hook,
                        transcript_text,
                        "https://api.openai.com/v1/chat/completions",
                        &key,
                        &model,
                        &[],
                    ).await.ok()
                } else { None }
            } else { None }
        }
        "groq" => {
            if let Ok(Some(key)) = crate::credentials::get(crate::credentials::GROQ) {
                if !key.trim().is_empty() {
                    let model = std::env::var("GROQ_MODEL").unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string());
                    generate_social_kit_openai_compat(
                        candidate_id,
                        hook,
                        transcript_text,
                        "https://api.groq.com/openai/v1/chat/completions",
                        &key,
                        &model,
                        &[],
                    ).await.ok()
                } else { None }
            } else { None }
        }
        "openrouter" => {
            if let Ok(Some(key)) = crate::credentials::get(crate::credentials::OPENROUTER) {
                if !key.trim().is_empty() {
                    let default_model = "google/gemini-2.5-flash".to_string();
                    let model = model_name
                        .filter(|m| !m.trim().is_empty())
                        .map(|m| m.trim().to_string())
                        .or_else(|| std::env::var("OPENROUTER_MODEL").ok())
                        .unwrap_or(default_model);
                    generate_social_kit_openai_compat(
                        candidate_id,
                        hook,
                        transcript_text,
                        "https://openrouter.ai/api/v1/chat/completions",
                        &key,
                        &model,
                        &[("HTTP-Referer", "https://github.com/subha-3128/ClipOn"), ("X-Title", "ClipOn")],
                    ).await.ok()
                } else { None }
            } else { None }
        }
        "local" | "ollama" => {
            let env_model = std::env::var("OLLAMA_MODEL").ok();
            let model = model_name
                .filter(|m| !m.trim().is_empty())
                .or(env_model.as_deref())
                .unwrap_or("llama3.2");
            generate_social_kit_with_local(candidate_id, hook, transcript_text, model).await.ok()
        }
        _ => {
            if let Ok(Some(key)) = crate::credentials::get(crate::credentials::DEEPSEEK) {
                if !key.trim().is_empty() {
                    generate_social_kit_with_deepseek(candidate_id, hook, transcript_text, &key).await.ok()
                } else { None }
            } else { None }
        }
    };

    res.unwrap_or_else(|| generate_heuristic_social_kit(candidate_id, hook, transcript_text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TranscriptSegment;

    #[test]
    fn test_parse_candidate_json_strict_validation() {
        let json_data = r#"{
            "candidates": [
                {
                    "start": 10.0,
                    "end": 45.0,
                    "score": 9.2,
                    "hook": "The secret to scaling",
                    "rationale": "High engagement topic"
                },
                {
                    "start": "bad_number",
                    "end": 50.0,
                    "score": 8.0,
                    "hook": "Corrupted start",
                    "rationale": "Should be rejected, not default to 0.0"
                },
                {
                    "start": 60.0,
                    "end": 55.0,
                    "score": 7.5,
                    "hook": "End before start",
                    "rationale": "Should be rejected"
                },
                {
                    "start": -5.0,
                    "end": 30.0,
                    "score": 6.0,
                    "hook": "Negative start",
                    "rationale": "Should be rejected"
                },
                {
                    "start": 70.0,
                    "end": 100.0,
                    "score": 8.5,
                    "hook": "",
                    "rationale": "Empty hook should be rejected"
                },
                {
                    "start": 800.0,
                    "end": 840.0,
                    "score": 8.0,
                    "hook": "Past timeline bounds",
                    "rationale": "Should be rejected"
                }
            ]
        }"#;

        let results = parse_candidate_json(json_data, 15.0, 300.0).expect("parsing should succeed");
        // Only the first candidate is valid!
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].start, 10.0);
        assert_eq!(results[0].end, 45.0);
        assert!((results[0].score - 0.92).abs() < 1e-4);
        assert_eq!(results[0].hook, "The secret to scaling");
    }

    #[test]
    fn test_chunk_transcript_no_data_loss() {
        let mut segments = Vec::new();
        for i in 0..1000 {
            let start = i as f64 * 3.0;
            let end = start + 2.8;
            segments.push(TranscriptSegment {
                start,
                end,
                speaker: Some(format!("Speaker {}", i % 2)),
                text: format!("Sentence {i} containing valuable conversation."),
            });
        }
        let transcript = NormalizedTranscript {
            language: "en".to_string(),
            duration: 3000.0,
            speakers: vec!["Speaker 0".to_string(), "Speaker 1".to_string()],
            words: Vec::new(),
            segments,
        };

        let chunks = chunk_transcript(&transcript, 1200.0, 90.0);
        assert!(chunks.len() >= 3);

        // Verify that every segment in the original transcript appears in at least one chunk
        let mut covered = vec![false; 1000];
        for chunk in &chunks {
            for seg in &chunk.segments {
                let idx = (seg.start / 3.0).round() as usize;
                if idx < 1000 {
                    covered[idx] = true;
                }
            }
        }
        assert!(covered.iter().all(|&c| c), "Every sentence in the transcript must be preserved without dropping");
    }

    #[test]
    fn test_deduplicate_and_rank_candidates() {
        let candidates = vec![
            CandidateDraft {
                start: 10.0,
                end: 40.0,
                score: 0.65,
                hook: "Lower score overlapping clip".to_string(),
                rationale: "Rationale 1".to_string(),
            },
            CandidateDraft {
                start: 12.0,
                end: 42.0,
                score: 0.95,
                hook: "Higher score overlapping clip".to_string(),
                rationale: "Rationale 2".to_string(),
            },
            CandidateDraft {
                start: 120.0,
                end: 150.0,
                score: 0.80,
                hook: "Completely separate clip".to_string(),
                rationale: "Rationale 3".to_string(),
            },
        ];

        let deduplicated = deduplicate_and_rank_candidates(candidates, 10);
        assert_eq!(deduplicated.len(), 2);
        assert_eq!(deduplicated[0].score, 0.95);
        assert_eq!(deduplicated[0].hook, "Higher score overlapping clip");
        assert_eq!(deduplicated[1].score, 0.80);
    }

    #[test]
    fn test_resolve_gemini_models() {
        let models = resolve_gemini_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"gemini-2.0-flash".to_string()));
        assert!(models.contains(&"gemini-1.5-flash".to_string()));
    }

    struct MockProvider {
        response: String,
    }

    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        async fn complete(
            &self,
            _prompt: &str,
            _json_mode: bool,
            _temperature: f64,
        ) -> Result<String> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn test_detect_candidates_with_provider_mock() {
        let mock_json = r#"{
            "candidates": [
                {
                    "start": 5.0,
                    "end": 35.0,
                    "score": 0.88,
                    "hook": "Unbelievable secret!",
                    "rationale": "Great hook and retention."
                }
            ]
        }"#;

        let provider = MockProvider {
            response: mock_json.to_string(),
        };
        let transcript = NormalizedTranscript {
            language: "en".into(),
            duration: 100.0,
            speakers: vec!["S1".into()],
            words: vec![],
            segments: vec![TranscriptSegment {
                start: 0.0,
                end: 50.0,
                speaker: Some("S1".into()),
                text: "Hello world, this is a test.".into(),
            }],
        };

        let drafts = detect_candidates_with_provider(&provider, &transcript)
            .await
            .unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].hook, "Unbelievable secret!");
        assert_eq!(drafts[0].score, 0.88);
    }
}

