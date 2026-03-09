use futures::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Result from an LLM call, including token usage for training data recording.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
}

/// A tool the LLM model can call during generation.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// A tool call requested by the model.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Response from a single LLM turn — either text or tool-use requests.
#[derive(Debug)]
pub enum LlmTurn {
    Text(LlmResponse),
    ToolUse {
        calls: Vec<ToolCall>,
        /// Reasoning text the model emitted alongside tool calls (preserved for context).
        partial_text: String,
        tokens_in: Option<u32>,
        tokens_out: Option<u32>,
    },
}

#[derive(Clone)]
pub struct LlmClient {
    client: Client,
    provider: String,
    model: String,
    api_key: String,
    max_output_tokens: u32,
    temperature: Option<f32>,
}

/// Returns true for OpenAI models that use reasoning/thinking tokens (o-series, GPT-5.x).
/// These models require `max_completion_tokens` instead of `max_tokens`.
fn is_openai_reasoning_model(model: &str) -> bool {
    model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4")
        || model.contains("gpt-5")
}

/// Returns true for Anthropic models that support extended thinking.
fn is_anthropic_thinking_model(model: &str) -> bool {
    model.contains("claude-sonnet-4") || model.contains("claude-opus-4")
}

/// Returns true for Gemini 3+ models that support explicit thinking controls.
fn is_gemini_thinking_model(model: &str) -> bool {
    normalize_gemini_model(model).starts_with("gemini-3")
}

/// Gemini direct API expects bare model IDs, while OpenRouter uses `google/...` slugs.
fn normalize_gemini_model(model: &str) -> &str {
    model.strip_prefix("google/").unwrap_or(model)
}

/// Minimum budget to enable extended thinking (thinking needs headroom beyond the text response).
const THINKING_BUDGET_THRESHOLD: u32 = 8_000;

impl LlmClient {
    pub fn new(provider: &str, model: &str, api_key: &str) -> Self {
        Self::with_budget(provider, model, api_key, 4096)
    }

    pub fn with_budget(provider: &str, model: &str, api_key: &str, max_output_tokens: u32) -> Self {
        let client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(300)) // 5 min for reasoning models
            .build()
            .unwrap_or_default();
        Self {
            client,
            provider: provider.to_string(),
            model: model.to_string(),
            api_key: api_key.to_string(),
            max_output_tokens,
            temperature: None,
        }
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn model_name(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    fn wants_json_mode(&self) -> bool {
        // All OpenAI-compatible providers support response_format json_object.
        // Without this, non-OpenAI models may return extra prose that fails parsing.
        matches!(self.provider.as_str(), "openai" | "openrouter" | "gemini")
    }

    fn with_openrouter_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.provider != "openrouter" {
            return builder;
        }

        let mut builder = builder;
        if let Ok(site_url) = std::env::var("OPENROUTER_HTTP_REFERER") {
            if !site_url.trim().is_empty() {
                builder = builder.header("HTTP-Referer", site_url);
            }
        }
        if let Ok(app_name) = std::env::var("OPENROUTER_APP_NAME") {
            if !app_name.trim().is_empty() {
                builder = builder.header("X-Title", app_name);
            }
        }
        builder
    }

    pub async fn complete(&self, prompt: &str) -> Result<LlmResponse, reqwest::Error> {
        match self.provider.as_str() {
            "anthropic" => self.complete_anthropic(prompt).await,
            "chatgpt" => self.complete_chatgpt(prompt).await,
            "openai" => {
                self.complete_openai(prompt, "https://api.openai.com/v1/chat/completions")
                    .await
            }
            "openrouter" => {
                self.complete_openai(prompt, "https://openrouter.ai/api/v1/chat/completions")
                    .await
            }
            "gemini" => {
                self.complete_openai(
                    prompt,
                    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                )
                .await
            }
            _ => Ok(LlmResponse {
                text: format!("Unsupported provider: {}", self.provider),
                tokens_in: None,
                tokens_out: None,
            }),
        }
    }

    /// Streaming completion that calls `on_token` for each text chunk as it arrives.
    pub async fn complete_streaming<F>(
        &self,
        prompt: &str,
        on_token: F,
    ) -> Result<LlmResponse, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let abort = Arc::new(AtomicBool::new(false));
        self.complete_streaming_abortable(prompt, on_token, abort)
            .await
    }

    /// Like `complete_streaming` but accepts an `Arc<AtomicBool>` abort flag.
    /// When the flag becomes `true` mid-stream (e.g. set by a repetition detector),
    /// the stream is terminated immediately and `Err("stream_aborted:…")` is returned.
    pub async fn complete_streaming_abortable<F>(
        &self,
        prompt: &str,
        on_token: F,
        abort: Arc<AtomicBool>,
    ) -> Result<LlmResponse, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        match self.provider.as_str() {
            "anthropic" => self.stream_anthropic(prompt, on_token, &abort).await,
            "chatgpt" => self.stream_chatgpt(prompt, on_token, &abort).await,
            "openai" => {
                self.stream_openai(
                    prompt,
                    "https://api.openai.com/v1/chat/completions",
                    on_token,
                    &abort,
                )
                .await
            }
            "openrouter" => {
                self.stream_openai(
                    prompt,
                    "https://openrouter.ai/api/v1/chat/completions",
                    on_token,
                    &abort,
                )
                .await
            }
            "gemini" => {
                self.stream_openai(
                    prompt,
                    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                    on_token,
                    &abort,
                )
                .await
            }
            _ => Ok(LlmResponse {
                text: format!("Unsupported provider: {}", self.provider),
                tokens_in: None,
                tokens_out: None,
            }),
        }
    }

    async fn complete_anthropic(&self, prompt: &str) -> Result<LlmResponse, reqwest::Error> {
        #[derive(Deserialize)]
        struct Resp {
            content: Vec<Block>,
            usage: Option<Usage>,
        }
        #[derive(Deserialize)]
        struct Block {
            #[serde(rename = "type")]
            block_type: Option<String>,
            text: Option<String>,
        }
        #[derive(Deserialize)]
        struct Usage {
            input_tokens: Option<u32>,
            output_tokens: Option<u32>,
        }

        let use_thinking = is_anthropic_thinking_model(&self.model)
            && self.max_output_tokens >= THINKING_BUDGET_THRESHOLD;

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_output_tokens,
            "messages": [{"role": "user", "content": prompt}],
        });

        // Anthropic thinking models don't support temperature; non-thinking models do
        if use_thinking {
            let thinking_budget = self.max_output_tokens.saturating_sub(4096).max(1024);
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": thinking_budget,
            });
            tracing::debug!(
                "Anthropic thinking enabled: budget_tokens={}",
                thinking_budget
            );
        } else if let Some(temp) = self.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        let resp: Resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        // Only collect text blocks (skip thinking blocks)
        let text = resp
            .content
            .into_iter()
            .filter(|c| c.block_type.as_deref() != Some("thinking"))
            .filter_map(|c| c.text)
            .collect::<Vec<_>>()
            .join("");
        let (tokens_in, tokens_out) = match resp.usage {
            Some(u) => (u.input_tokens, u.output_tokens),
            None => (None, None),
        };
        tracing::debug!(tokens_in = ?tokens_in, tokens_out = ?tokens_out, thinking = use_thinking, "Anthropic usage");
        Ok(LlmResponse {
            text,
            tokens_in,
            tokens_out,
        })
    }

    async fn stream_anthropic<F>(
        &self,
        prompt: &str,
        on_token: F,
        abort: &AtomicBool,
    ) -> Result<LlmResponse, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let use_thinking = is_anthropic_thinking_model(&self.model)
            && self.max_output_tokens >= THINKING_BUDGET_THRESHOLD;

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_output_tokens,
            "stream": true,
            "messages": [{"role": "user", "content": prompt}],
        });

        if use_thinking {
            let thinking_budget = self.max_output_tokens.saturating_sub(4096).max(1024);
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": thinking_budget,
            });
        } else if let Some(temp) = self.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        // Check for API errors before trying to stream
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(&body)
                    .to_string()
            } else {
                body
            };
            tracing::error!("Anthropic API error {}: {}", status, err_msg);
            return Err(format!("Anthropic API error {}: {}", status, err_msg));
        }

        let mut full_text = String::new();
        let mut tokens_in: Option<u32> = None;
        let mut tokens_out: Option<u32> = None;
        let mut buffer = String::new();
        // Track whether current content block is thinking or text
        let mut in_thinking_block = false;

        let mut stream = resp.bytes_stream();
        'stream: while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match event_type {
                            "content_block_start" => {
                                // Detect thinking vs text blocks
                                let block_type = event
                                    .get("content_block")
                                    .and_then(|b| b.get("type"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("text");
                                in_thinking_block = block_type == "thinking";
                            }
                            "content_block_delta" => {
                                let delta_type = event
                                    .get("delta")
                                    .and_then(|d| d.get("type"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");

                                if delta_type == "thinking_delta" {
                                    // Thinking tokens — emit to frontend (keeps spinner alive)
                                    // but don't include in full_text (only text blocks matter).
                                    // We do NOT abort on thinking token repetition — extended
                                    // thinking blocks can legitimately be repetitive.
                                    if let Some(thinking) = event
                                        .get("delta")
                                        .and_then(|d| d.get("thinking"))
                                        .and_then(|t| t.as_str())
                                    {
                                        on_token(thinking);
                                    }
                                } else if delta_type == "text_delta" || !in_thinking_block {
                                    // Text content — this is the actual response
                                    if let Some(text) = event
                                        .get("delta")
                                        .and_then(|d| d.get("text"))
                                        .and_then(|t| t.as_str())
                                    {
                                        full_text.push_str(text);
                                        on_token(text);
                                        if abort.load(Ordering::Relaxed) {
                                            tracing::warn!(
                                                "stream_anthropic: repetition abort after {} chars",
                                                full_text.len()
                                            );
                                            break 'stream;
                                        }
                                    }
                                }
                            }
                            "content_block_stop" => {
                                in_thinking_block = false;
                            }
                            "message_delta" => {
                                if let Some(usage) = event.get("usage") {
                                    tokens_out = usage
                                        .get("output_tokens")
                                        .and_then(|t| t.as_u64())
                                        .map(|t| t as u32);
                                }
                            }
                            "message_start" => {
                                if let Some(usage) =
                                    event.get("message").and_then(|m| m.get("usage"))
                                {
                                    tokens_in = usage
                                        .get("input_tokens")
                                        .and_then(|t| t.as_u64())
                                        .map(|t| t as u32);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if abort.load(Ordering::Relaxed) {
            return Err(format!(
                "stream_aborted:repetition_detected:{} chars accumulated",
                full_text.len()
            ));
        }
        tracing::debug!(tokens_in = ?tokens_in, tokens_out = ?tokens_out, thinking = use_thinking, "Anthropic streaming usage");
        Ok(LlmResponse {
            text: full_text,
            tokens_in,
            tokens_out,
        })
    }

    /// OpenAI-compatible completions (works for OpenAI and OpenRouter).
    /// Handles reasoning models (o-series, GPT-5.x) which use `max_completion_tokens`.
    async fn complete_openai(
        &self,
        prompt: &str,
        url: &str,
    ) -> Result<LlmResponse, reqwest::Error> {
        #[derive(Deserialize)]
        struct Resp {
            choices: Vec<Choice>,
            usage: Option<Usage>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: ChoiceMsg,
        }
        #[derive(Deserialize)]
        struct ChoiceMsg {
            content: Option<String>,
            reasoning_content: Option<String>, // thinking models (Qwen3.5, etc.)
        }
        #[derive(Deserialize)]
        struct Usage {
            prompt_tokens: Option<u32>,
            completion_tokens: Option<u32>,
        }

        let request_model = if self.provider == "gemini" {
            normalize_gemini_model(&self.model)
        } else {
            self.model.as_str()
        };

        let mut body = serde_json::json!({
            "model": request_model,
            "messages": [{"role": "user", "content": prompt}],
        });

        if is_openai_reasoning_model(&self.model) {
            body["max_completion_tokens"] = serde_json::json!(self.max_output_tokens);
            // Reasoning models (o-series, GPT-5.x) don't support temperature or response_format
        } else {
            body["max_tokens"] = serde_json::json!(self.max_output_tokens);
            if self.wants_json_mode() {
                body["response_format"] = serde_json::json!({ "type": "json_object" });
            }
            if let Some(temp) = self.temperature {
                body["temperature"] = serde_json::json!(temp);
            }
        }

        if self.provider == "gemini" && is_gemini_thinking_model(request_model) {
            body["extra_body"] = serde_json::json!({
                "google": {
                    "thinking_config": {
                        "thinking_budget": if self.max_output_tokens > THINKING_BUDGET_THRESHOLD { "high" } else { "low" },
                        "include_thoughts": false
                    }
                }
            });
        }

        let req = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body);
        let resp: Resp = self
            .with_openrouter_headers(req)
            .send()
            .await?
            .json()
            .await?;

        let text = resp
            .choices
            .into_iter()
            .filter_map(|c| c.message.content.or(c.message.reasoning_content))
            .collect::<Vec<_>>()
            .join("");
        let (tokens_in, tokens_out) = match resp.usage {
            Some(u) => (u.prompt_tokens, u.completion_tokens),
            None => (None, None),
        };
        tracing::debug!(tokens_in = ?tokens_in, tokens_out = ?tokens_out, "OpenAI usage");
        Ok(LlmResponse {
            text,
            tokens_in,
            tokens_out,
        })
    }

    async fn stream_openai<F>(
        &self,
        prompt: &str,
        url: &str,
        on_token: F,
        abort: &AtomicBool,
    ) -> Result<LlmResponse, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let request_model = if self.provider == "gemini" {
            normalize_gemini_model(&self.model)
        } else {
            self.model.as_str()
        };

        let mut body = serde_json::json!({
            "model": request_model,
            "stream": true,
            "messages": [{"role": "user", "content": prompt}],
        });
        if self.provider != "gemini" {
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }

        if is_openai_reasoning_model(&self.model) {
            body["max_completion_tokens"] = serde_json::json!(self.max_output_tokens);
            // Reasoning models (o-series, GPT-5.x) don't support temperature or response_format
        } else {
            body["max_tokens"] = serde_json::json!(self.max_output_tokens);
            if self.wants_json_mode() {
                body["response_format"] = serde_json::json!({ "type": "json_object" });
            }
            if let Some(temp) = self.temperature {
                body["temperature"] = serde_json::json!(temp);
            }
        }

        if self.provider == "gemini" && is_gemini_thinking_model(request_model) {
            body["extra_body"] = serde_json::json!({
                "google": {
                    "thinking_config": {
                        "thinking_budget": if self.max_output_tokens > THINKING_BUDGET_THRESHOLD { "high" } else { "low" },
                        "include_thoughts": false
                    }
                }
            });
        }

        let req = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body);
        let resp = self
            .with_openrouter_headers(req)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        // Check for API errors before trying to stream
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(&body)
                    .to_string()
            } else {
                body
            };
            tracing::error!("OpenAI API error {}: {}", status, err_msg);
            return Err(format!("OpenAI API error {}: {}", status, err_msg));
        }

        let mut full_text = String::new();
        let mut tokens_in: Option<u32> = None;
        let mut tokens_out: Option<u32> = None;
        let mut buffer = String::new();

        let mut stream = resp.bytes_stream();
        'stream: while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                        // Standard content delta.
                        // full_text accumulates everything (needed for JSON parsing).
                        // on_token only fires for non-whitespace content — prevents
                        // blank-line spam when reasoning models emit delta.content=""
                        // or whitespace-only chunks during thinking phases.
                        if let Some(text) = event
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("content"))
                            .and_then(|t| t.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            full_text.push_str(text);
                            if !text.chars().all(|c| c.is_ascii_whitespace()) {
                                on_token(text);
                                if abort.load(Ordering::Relaxed) {
                                    tracing::warn!(
                                        "stream_openai: repetition abort after {} chars",
                                        full_text.len()
                                    );
                                    break 'stream;
                                }
                            }
                        }
                        // Usage chunk (sent at end with stream_options.include_usage)
                        if let Some(usage) = event.get("usage") {
                            tokens_in = usage
                                .get("prompt_tokens")
                                .and_then(|t| t.as_u64())
                                .map(|t| t as u32);
                            tokens_out = usage
                                .get("completion_tokens")
                                .and_then(|t| t.as_u64())
                                .map(|t| t as u32);
                        }
                    }
                }
            }
        }

        if abort.load(Ordering::Relaxed) {
            return Err(format!(
                "stream_aborted:repetition_detected:{} chars accumulated",
                full_text.len()
            ));
        }
        tracing::debug!(tokens_in = ?tokens_in, tokens_out = ?tokens_out, "OpenAI streaming complete, {} chars", full_text.len());
        Ok(LlmResponse {
            text: full_text,
            tokens_in,
            tokens_out,
        })
    }

    // ── ChatGPT Subscription (Responses API) ────────────────────────

    /// Build headers for ChatGPT subscription API requests.
    fn chatgpt_headers(&self) -> Vec<(&'static str, String)> {
        let account_id = crate::api::chatgpt_oauth::get_account_id();
        let mut headers = vec![
            ("Authorization", format!("Bearer {}", self.api_key)),
            ("content-type", "application/json".to_string()),
            ("accept", "text/event-stream".to_string()),
            ("originator", "codex_cli_rs".to_string()),
            (
                "user-agent",
                "codex_cli_rs/0.0.0 (Windows 10; x86_64) ChatDB".to_string(),
            ),
        ];
        if let Some(aid) = account_id {
            headers.push(("ChatGPT-Account-Id", aid));
        }
        headers
    }

    /// Build Responses API request body from a prompt.
    fn chatgpt_body(&self, prompt: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "instructions": "You are a mathematical proof assistant. Respond with precise, structured JSON when asked. Follow all formatting instructions exactly.",
            "input": [{"role": "user", "content": prompt}],
            "stream": true,
            "store": false,
            "include": ["reasoning.encrypted_content"],
        })
    }

    /// Build Responses API request body with tool definitions.
    /// Converts Chat Completions-style messages to Responses API input format:
    /// - role:"user" → {role: "user", content: "..."}
    /// - role:"assistant" with tool_calls → {type: "function_call", call_id, name, arguments} per call
    /// - role:"tool" → {type: "function_call_output", call_id, output}
    fn chatgpt_body_with_tools(
        &self,
        messages: &[serde_json::Value],
        tools: &[ToolDef],
    ) -> serde_json::Value {
        // Convert messages to Responses API input items
        let mut input: Vec<serde_json::Value> = Vec::new();
        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            match role {
                "user" => {
                    let content = msg.get("content");
                    // Anthropic format: content can be array of tool_result objects
                    if let Some(arr) = content.and_then(|c| c.as_array()) {
                        for item in arr {
                            let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if item_type == "tool_result" {
                                let call_id = item
                                    .get("tool_use_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let output =
                                    item.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                input.push(serde_json::json!({
                                    "type": "function_call_output",
                                    "call_id": call_id,
                                    "output": output,
                                }));
                            }
                        }
                    } else if let Some(text) = content.and_then(|c| c.as_str()) {
                        input.push(serde_json::json!({"role": "user", "content": text}));
                    }
                }
                "assistant" => {
                    // May have tool_calls (OpenAI format) or content array (Anthropic format)
                    if let Some(tool_calls) = msg.get("tool_calls").and_then(|t| t.as_array()) {
                        for tc in tool_calls {
                            let call_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let func = tc.get("function");
                            let name = func
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("");
                            let arguments = func
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("{}");
                            input.push(serde_json::json!({
                                "type": "function_call",
                                "call_id": call_id,
                                "name": name,
                                "arguments": arguments,
                            }));
                        }
                    } else if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                        // Anthropic format: content array with text and tool_use blocks
                        for block in content {
                            let block_type =
                                block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if block_type == "tool_use" {
                                let call_id =
                                    block.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let arguments = serde_json::to_string(
                                    block.get("input").unwrap_or(&serde_json::json!({})),
                                )
                                .unwrap_or_default();
                                input.push(serde_json::json!({
                                    "type": "function_call",
                                    "call_id": call_id,
                                    "name": name,
                                    "arguments": arguments,
                                }));
                            }
                        }
                    }
                }
                "tool" => {
                    // OpenAI tool result format
                    let call_id = msg
                        .get("tool_call_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let output = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    input.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": output,
                    }));
                }
                _ => {}
            }
        }

        // Responses API tool format: flat {type, name, description, parameters}
        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                })
            })
            .collect();

        serde_json::json!({
            "model": self.model,
            "instructions": "You are a mathematical proof assistant. Respond with precise, structured JSON when asked. Follow all formatting instructions exactly. You have tools available — use them proactively to verify your work before submitting.",
            "input": input,
            "tools": tools_json,
            "tool_choice": "auto",
            "parallel_tool_calls": true,
            "stream": true,
            "store": false,
            "include": ["reasoning.encrypted_content"],
        })
    }

    /// Non-streaming completion via ChatGPT Responses API.
    /// (Always streams internally, buffers the full response.)
    async fn complete_chatgpt(&self, prompt: &str) -> Result<LlmResponse, reqwest::Error> {
        let body = self.chatgpt_body(prompt);
        let url = format!("{}/responses", crate::api::chatgpt_oauth::CHATGPT_API_BASE);

        let mut req = self.client.post(&url);
        for (k, v) in self.chatgpt_headers() {
            req = req.header(k, v);
        }
        let resp = req.json(&body).send().await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            tracing::error!("ChatGPT API error: {}", text);
            return Ok(LlmResponse {
                text: format!("ChatGPT API error: {text}"),
                tokens_in: None,
                tokens_out: None,
            });
        }

        // Buffer the streamed response
        let full_body = resp.text().await.unwrap_or_default();
        let (text, tokens_in, tokens_out) = Self::parse_chatgpt_sse(&full_body);

        tracing::debug!(tokens_in = ?tokens_in, tokens_out = ?tokens_out, "ChatGPT complete, {} chars", text.len());
        Ok(LlmResponse {
            text,
            tokens_in,
            tokens_out,
        })
    }

    /// Streaming completion via ChatGPT Responses API.
    async fn stream_chatgpt<F>(
        &self,
        prompt: &str,
        on_token: F,
        abort: &AtomicBool,
    ) -> Result<LlmResponse, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let body = self.chatgpt_body(prompt);
        let url = format!("{}/responses", crate::api::chatgpt_oauth::CHATGPT_API_BASE);

        let mut req = self.client.post(&url);
        for (k, v) in self.chatgpt_headers() {
            req = req.header(k, v);
        }
        let resp = req.json(&body).send().await.map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(&body)
                    .to_string()
            } else {
                body
            };
            tracing::error!("ChatGPT API error {}: {}", status, err_msg);
            return Err(format!("ChatGPT API error {status}: {err_msg}"));
        }

        let mut full_text = String::new();
        let mut tokens_in: Option<u32> = None;
        let mut tokens_out: Option<u32> = None;
        let mut buffer = String::new();

        let mut stream = resp.bytes_stream();
        'stream: while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break 'stream;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match event_type {
                            // Text output delta
                            "response.output_text.delta" => {
                                if let Some(text) = event.get("delta").and_then(|d| d.as_str()) {
                                    full_text.push_str(text);
                                    if !text.chars().all(|c| c.is_ascii_whitespace()) {
                                        on_token(text);
                                        if abort.load(Ordering::Relaxed) {
                                            tracing::warn!(
                                                "stream_chatgpt: repetition abort after {} chars",
                                                full_text.len()
                                            );
                                            break 'stream;
                                        }
                                    }
                                }
                            }
                            // Response completed — extract usage
                            "response.completed" => {
                                if let Some(response) = event.get("response") {
                                    if let Some(usage) = response.get("usage") {
                                        tokens_in = usage
                                            .get("input_tokens")
                                            .and_then(|t| t.as_u64())
                                            .map(|t| t as u32);
                                        tokens_out = usage
                                            .get("output_tokens")
                                            .and_then(|t| t.as_u64())
                                            .map(|t| t as u32);
                                    }
                                }
                            }
                            // Error
                            "response.failed" | "error" => {
                                let error_msg = event
                                    .get("error")
                                    .or_else(|| event.get("response").and_then(|r| r.get("error")))
                                    .and_then(|e| e.get("message"))
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Unknown error");
                                return Err(format!("ChatGPT API error: {error_msg}"));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if abort.load(Ordering::Relaxed) {
            return Err(format!(
                "stream_aborted:repetition_detected:{} chars accumulated",
                full_text.len()
            ));
        }
        tracing::debug!(tokens_in = ?tokens_in, tokens_out = ?tokens_out, "ChatGPT streaming complete, {} chars", full_text.len());
        Ok(LlmResponse {
            text: full_text,
            tokens_in,
            tokens_out,
        })
    }

    /// Parse ChatGPT SSE response body (buffered) into text + usage.
    fn parse_chatgpt_sse(body: &str) -> (String, Option<u32>, Option<u32>) {
        let mut text = String::new();
        let mut tokens_in = None;
        let mut tokens_out = None;

        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if event_type == "response.output_text.delta" {
                        if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                            text.push_str(delta);
                        }
                    } else if event_type == "response.completed" {
                        if let Some(usage) = event.get("response").and_then(|r| r.get("usage")) {
                            tokens_in = usage
                                .get("input_tokens")
                                .and_then(|t| t.as_u64())
                                .map(|t| t as u32);
                            tokens_out = usage
                                .get("output_tokens")
                                .and_then(|t| t.as_u64())
                                .map(|t| t as u32);
                        }
                    }
                }
            }
        }
        (text, tokens_in, tokens_out)
    }

    /// ChatGPT Responses API streaming with tool-use support.
    ///
    /// Handles SSE events:
    /// - `response.output_item.added` with `type: "function_call"` → start tracking tool call
    /// - `response.function_call_arguments.delta` → accumulate arguments JSON
    /// - `response.output_item.done` with `type: "function_call"` → finalize tool call
    /// - `response.output_text.delta` → text output (partial_text alongside tool calls)
    /// - `response.completed` → usage stats
    async fn stream_chatgpt_with_tools<F>(
        &self,
        messages: &[serde_json::Value],
        tools: &[ToolDef],
        on_token: F,
        abort: &AtomicBool,
    ) -> Result<LlmTurn, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        tracing::info!(
            model = %self.model,
            tools_count = tools.len(),
            messages_count = messages.len(),
            "stream_chatgpt_with_tools: {} tools={:?}, {} messages",
            tools.len(),
            tool_names,
            messages.len(),
        );
        let body = self.chatgpt_body_with_tools(messages, tools);
        tracing::debug!(
            "ChatGPT request body tools+input keys: tools={}, input_items={}",
            body.get("tools")
                .and_then(|t| t.as_array())
                .map(|a| a.len())
                .unwrap_or(0),
            body.get("input")
                .and_then(|i| i.as_array())
                .map(|a| a.len())
                .unwrap_or(0),
        );
        let url = format!("{}/responses", crate::api::chatgpt_oauth::CHATGPT_API_BASE);

        let mut req = self.client.post(&url);
        for (k, v) in self.chatgpt_headers() {
            req = req.header(k, v);
        }
        let resp = req.json(&body).send().await.map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(&body)
                    .to_string()
            } else {
                body
            };
            tracing::error!("ChatGPT API error {}: {}", status, err_msg);
            return Err(format!("ChatGPT API error {status}: {err_msg}"));
        }

        let mut full_text = String::new();
        let mut tokens_in: Option<u32> = None;
        let mut tokens_out: Option<u32> = None;
        let mut buffer = String::new();

        // Tool call tracking
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut current_call_id: Option<String> = None;
        let mut current_call_name: Option<String> = None;
        let mut current_args_buf = String::new();

        let mut stream = resp.bytes_stream();
        'stream: while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break 'stream;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match event_type {
                            // New output item — check if it's a function call
                            "response.output_item.added" => {
                                if let Some(item) = event.get("item") {
                                    let item_type =
                                        item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    if item_type == "function_call" {
                                        current_call_id = item
                                            .get("call_id")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                        current_call_name = item
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                        current_args_buf.clear();
                                        tracing::debug!(
                                            call_id = ?current_call_id,
                                            name = ?current_call_name,
                                            "ChatGPT function_call started"
                                        );
                                    }
                                }
                            }
                            // Streaming function call arguments
                            "response.function_call_arguments.delta" => {
                                if let Some(delta) = event.get("delta").and_then(|d| d.as_str()) {
                                    current_args_buf.push_str(delta);
                                }
                            }
                            // Function call arguments complete — finalize the tool call.
                            // This fires before output_item.done so we use it as the
                            // authoritative finalization point (per OpenAI docs).
                            "response.function_call_arguments.done" => {
                                // Prefer the full arguments string from this event
                                let args_str = event
                                    .get("arguments")
                                    .and_then(|a| a.as_str())
                                    .unwrap_or(&current_args_buf);
                                if let (Some(id), Some(name)) =
                                    (current_call_id.take(), current_call_name.take())
                                {
                                    let input: serde_json::Value = serde_json::from_str(args_str)
                                        .unwrap_or(serde_json::Value::Object(
                                            serde_json::Map::new(),
                                        ));
                                    tracing::debug!(
                                        call_id = %id,
                                        name = %name,
                                        input = %input,
                                        "ChatGPT tool call finalized"
                                    );
                                    tool_calls.push(ToolCall { id, name, input });
                                    current_args_buf.clear();
                                }
                            }
                            // output_item.done for function_call — already handled above,
                            // but skip gracefully if arguments.done didn't fire.
                            "response.output_item.done" => {
                                if current_call_id.is_some() {
                                    let is_fn = event
                                        .get("item")
                                        .and_then(|i| i.get("type"))
                                        .and_then(|t| t.as_str())
                                        == Some("function_call");
                                    if is_fn {
                                        if let (Some(id), Some(name)) =
                                            (current_call_id.take(), current_call_name.take())
                                        {
                                            let args_str = event
                                                .get("item")
                                                .and_then(|i| i.get("arguments"))
                                                .and_then(|a| a.as_str())
                                                .unwrap_or(&current_args_buf);
                                            let input: serde_json::Value = serde_json::from_str(
                                                args_str,
                                            )
                                            .unwrap_or(serde_json::Value::Object(
                                                serde_json::Map::new(),
                                            ));
                                            tracing::debug!(
                                                call_id = %id,
                                                name = %name,
                                                input = %input,
                                                "ChatGPT tool call finalized (fallback via output_item.done)"
                                            );
                                            tool_calls.push(ToolCall { id, name, input });
                                            current_args_buf.clear();
                                        }
                                    }
                                }
                            }
                            // Text output delta
                            "response.output_text.delta" => {
                                if let Some(text) = event.get("delta").and_then(|d| d.as_str()) {
                                    full_text.push_str(text);
                                    if !text.chars().all(|c| c.is_ascii_whitespace()) {
                                        on_token(text);
                                        if abort.load(Ordering::Relaxed) {
                                            tracing::warn!(
                                                "stream_chatgpt_with_tools: repetition abort after {} chars",
                                                full_text.len()
                                            );
                                            break 'stream;
                                        }
                                    }
                                }
                            }
                            // Response completed — extract usage
                            "response.completed" => {
                                if let Some(response) = event.get("response") {
                                    if let Some(usage) = response.get("usage") {
                                        tokens_in = usage
                                            .get("input_tokens")
                                            .and_then(|t| t.as_u64())
                                            .map(|t| t as u32);
                                        tokens_out = usage
                                            .get("output_tokens")
                                            .and_then(|t| t.as_u64())
                                            .map(|t| t as u32);
                                    }
                                }
                            }
                            // Error
                            "response.failed" | "error" => {
                                let error_msg = event
                                    .get("error")
                                    .or_else(|| event.get("response").and_then(|r| r.get("error")))
                                    .and_then(|e| e.get("message"))
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Unknown error");
                                return Err(format!("ChatGPT API error: {error_msg}"));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if abort.load(Ordering::Relaxed) {
            return Err(format!(
                "stream_aborted:repetition_detected:{} chars accumulated",
                full_text.len()
            ));
        }

        tracing::debug!(
            tokens_in = ?tokens_in,
            tokens_out = ?tokens_out,
            tool_calls = tool_calls.len(),
            "ChatGPT tool-use streaming complete"
        );

        if !tool_calls.is_empty() {
            Ok(LlmTurn::ToolUse {
                calls: tool_calls,
                partial_text: full_text,
                tokens_in,
                tokens_out,
            })
        } else {
            Ok(LlmTurn::Text(LlmResponse {
                text: full_text,
                tokens_in,
                tokens_out,
            }))
        }
    }

    // ── Tool-use support ──────────────────────────────────────────────

    /// Completion that allows the model to call tools.
    ///
    /// `messages` should be pre-built role-tagged JSON values (same schema the
    /// providers expect).  `tools` describes the functions the model may invoke.
    /// `on_token` fires for text content and thinking tokens (keeps the frontend
    /// spinner alive) but **not** for tool-call argument chunks.
    ///
    /// Returns `LlmTurn::ToolUse` when the model requests one or more tool
    /// calls, or `LlmTurn::Text` when it produces a plain text response.
    pub async fn complete_with_tools<F>(
        &self,
        messages: &[serde_json::Value],
        tools: &[ToolDef],
        on_token: F,
    ) -> Result<LlmTurn, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let abort = Arc::new(AtomicBool::new(false));
        self.complete_with_tools_abortable(messages, tools, on_token, abort)
            .await
    }

    /// Like `complete_with_tools` but accepts an `Arc<AtomicBool>` abort flag.
    /// When the flag becomes `true` mid-stream the stream is terminated immediately.
    pub async fn complete_with_tools_abortable<F>(
        &self,
        messages: &[serde_json::Value],
        tools: &[ToolDef],
        on_token: F,
        abort: Arc<AtomicBool>,
    ) -> Result<LlmTurn, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        // Fall back to simple text completion when there are no tools or the
        // model is a reasoning model that doesn't support tool use.
        // ChatGPT provider uses the Responses API which supports tools for all models.
        // ChatGPT Codex backend (chatgpt.com/backend-api/codex) only supports its
        // built-in tool types (local_shell, web_search) — custom function tools are
        // silently ignored. Skip tool machinery for chatgpt provider.
        let skip_tools = tools.is_empty()
            || self.provider == "chatgpt"
            || is_openai_reasoning_model(&self.model);
        if skip_tools {
            let prompt = messages
                .first()
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("");
            let resp = self
                .complete_streaming_abortable(prompt, on_token, abort)
                .await?;
            return Ok(LlmTurn::Text(resp));
        }

        match self.provider.as_str() {
            "anthropic" => {
                self.stream_anthropic_with_tools(messages, tools, on_token, &abort)
                    .await
            }
            "chatgpt" => {
                self.stream_chatgpt_with_tools(messages, tools, on_token, &abort)
                    .await
            }
            "openai" => {
                self.stream_openai_with_tools(
                    messages,
                    tools,
                    "https://api.openai.com/v1/chat/completions",
                    on_token,
                    &abort,
                )
                .await
            }
            "openrouter" => {
                self.stream_openai_with_tools(
                    messages,
                    tools,
                    "https://openrouter.ai/api/v1/chat/completions",
                    on_token,
                    &abort,
                )
                .await
            }
            "gemini" => {
                self.stream_openai_with_tools(
                    messages,
                    tools,
                    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                    on_token,
                    &abort,
                )
                .await
            }
            _ => Ok(LlmTurn::Text(LlmResponse {
                text: format!("Unsupported provider: {}", self.provider),
                tokens_in: None,
                tokens_out: None,
            })),
        }
    }

    /// Anthropic streaming with tool-use support.
    async fn stream_anthropic_with_tools<F>(
        &self,
        messages: &[serde_json::Value],
        tools: &[ToolDef],
        on_token: F,
        abort: &AtomicBool,
    ) -> Result<LlmTurn, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let use_thinking = is_anthropic_thinking_model(&self.model)
            && self.max_output_tokens >= THINKING_BUDGET_THRESHOLD;

        // Build the tools array in Anthropic's format
        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_output_tokens,
            "stream": true,
            "messages": messages,
            "tools": tools_json,
        });

        if use_thinking {
            let thinking_budget = self.max_output_tokens.saturating_sub(4096).max(1024);
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": thinking_budget,
            });
        } else if let Some(temp) = self.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        // Check for API errors before trying to stream
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(&body)
                    .to_string()
            } else {
                body
            };
            tracing::error!("Anthropic API error {}: {}", status, err_msg);
            return Err(format!("Anthropic API error {}: {}", status, err_msg));
        }

        let mut full_text = String::new();
        let mut tokens_in: Option<u32> = None;
        let mut tokens_out: Option<u32> = None;
        let mut buffer = String::new();
        let mut in_thinking_block = false;

        // Tool-use tracking: each tool_use content block gets its own entry
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut current_tool_id: Option<String> = None;
        let mut current_tool_name: Option<String> = None;
        let mut current_tool_json_buf = String::new();

        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match event_type {
                            "content_block_start" => {
                                let block = event.get("content_block");
                                let block_type = block
                                    .and_then(|b| b.get("type"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("text");

                                in_thinking_block = block_type == "thinking";

                                if block_type == "tool_use" {
                                    // Start tracking a new tool call
                                    current_tool_id = block
                                        .and_then(|b| b.get("id"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    current_tool_name = block
                                        .and_then(|b| b.get("name"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string());
                                    current_tool_json_buf.clear();
                                    tracing::debug!(
                                        tool_id = ?current_tool_id,
                                        tool_name = ?current_tool_name,
                                        "Anthropic tool_use block started"
                                    );
                                }
                            }
                            "content_block_delta" => {
                                let delta_type = event
                                    .get("delta")
                                    .and_then(|d| d.get("type"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");

                                match delta_type {
                                    "thinking_delta" => {
                                        // Thinking tokens — emit to frontend but skip full_text.
                                        // Do NOT abort on thinking token repetition.
                                        if let Some(thinking) = event
                                            .get("delta")
                                            .and_then(|d| d.get("thinking"))
                                            .and_then(|t| t.as_str())
                                        {
                                            on_token(thinking);
                                        }
                                    }
                                    "text_delta" => {
                                        if let Some(text) = event
                                            .get("delta")
                                            .and_then(|d| d.get("text"))
                                            .and_then(|t| t.as_str())
                                        {
                                            full_text.push_str(text);
                                            on_token(text);
                                            if abort.load(Ordering::Relaxed) {
                                                tracing::warn!("stream_anthropic_with_tools: repetition abort after {} chars", full_text.len());
                                                return Err(format!("stream_aborted:repetition_detected:{} chars accumulated", full_text.len()));
                                            }
                                        }
                                    }
                                    "input_json_delta" => {
                                        // Tool call argument fragment — accumulate, don't fire on_token
                                        if let Some(partial) = event
                                            .get("delta")
                                            .and_then(|d| d.get("partial_json"))
                                            .and_then(|t| t.as_str())
                                        {
                                            current_tool_json_buf.push_str(partial);
                                        }
                                    }
                                    _ => {
                                        // Fallback for non-thinking text content
                                        if !in_thinking_block {
                                            if let Some(text) = event
                                                .get("delta")
                                                .and_then(|d| d.get("text"))
                                                .and_then(|t| t.as_str())
                                            {
                                                full_text.push_str(text);
                                                on_token(text);
                                            }
                                        }
                                    }
                                }
                            }
                            "content_block_stop" => {
                                // If we were accumulating a tool call, finalize it
                                if let (Some(id), Some(name)) =
                                    (current_tool_id.take(), current_tool_name.take())
                                {
                                    let input: serde_json::Value = serde_json::from_str(
                                        &current_tool_json_buf,
                                    )
                                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                                    tracing::debug!(
                                        tool_id = %id,
                                        tool_name = %name,
                                        input = %input,
                                        "Anthropic tool call finalized"
                                    );
                                    tool_calls.push(ToolCall { id, name, input });
                                    current_tool_json_buf.clear();
                                }
                                in_thinking_block = false;
                            }
                            "message_delta" => {
                                if let Some(usage) = event.get("usage") {
                                    tokens_out = usage
                                        .get("output_tokens")
                                        .and_then(|t| t.as_u64())
                                        .map(|t| t as u32);
                                }
                            }
                            "message_start" => {
                                if let Some(usage) =
                                    event.get("message").and_then(|m| m.get("usage"))
                                {
                                    tokens_in = usage
                                        .get("input_tokens")
                                        .and_then(|t| t.as_u64())
                                        .map(|t| t as u32);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        tracing::debug!(
            tokens_in = ?tokens_in,
            tokens_out = ?tokens_out,
            tool_calls = tool_calls.len(),
            thinking = use_thinking,
            "Anthropic tool-use streaming complete"
        );

        if !tool_calls.is_empty() {
            Ok(LlmTurn::ToolUse {
                calls: tool_calls,
                partial_text: full_text,
                tokens_in,
                tokens_out,
            })
        } else {
            Ok(LlmTurn::Text(LlmResponse {
                text: full_text,
                tokens_in,
                tokens_out,
            }))
        }
    }

    /// OpenAI-compatible streaming with tool-use support.
    async fn stream_openai_with_tools<F>(
        &self,
        messages: &[serde_json::Value],
        tools: &[ToolDef],
        url: &str,
        on_token: F,
        abort: &AtomicBool,
    ) -> Result<LlmTurn, String>
    where
        F: Fn(&str) + Send + 'static,
    {
        let request_model = if self.provider == "gemini" {
            normalize_gemini_model(&self.model)
        } else {
            self.model.as_str()
        };

        // Build the tools array in OpenAI's format
        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": request_model,
            "stream": true,
            "messages": messages,
            "tools": tools_json,
        });
        if self.provider != "gemini" {
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }

        // Reasoning models already handled by fallback in complete_with_tools,
        // but set token limits consistently for non-reasoning models.
        body["max_tokens"] = serde_json::json!(self.max_output_tokens);
        if let Some(temp) = self.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        if self.provider == "gemini" && is_gemini_thinking_model(request_model) {
            body["extra_body"] = serde_json::json!({
                "google": {
                    "thinking_config": {
                        "thinking_budget": if self.max_output_tokens > THINKING_BUDGET_THRESHOLD { "high" } else { "low" },
                        "include_thoughts": false
                    }
                }
            });
        }

        let req = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body);
        let resp = self
            .with_openrouter_headers(req)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        // Check for API errors before trying to stream
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let err_msg = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                v.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or(&body)
                    .to_string()
            } else {
                body
            };
            tracing::error!("OpenAI API error {}: {}", status, err_msg);
            return Err(format!("OpenAI API error {}: {}", status, err_msg));
        }

        let mut full_text = String::new();
        let mut tokens_in: Option<u32> = None;
        let mut tokens_out: Option<u32> = None;
        let mut buffer = String::new();

        // Tool call tracking: Vec of (id, name, arguments_buffer) indexed by tool_call index
        let mut pending_tool_calls: Vec<(String, String, String)> = Vec::new();

        let mut stream = resp.bytes_stream();
        'stream: while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| e.to_string())?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim_end_matches('\r').to_string();
                buffer = buffer[line_end + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
                        // Standard text content delta.
                        // full_text accumulates everything (needed for JSON / tool arg parsing).
                        // on_token only fires for non-whitespace — prevents blank-line spam
                        // from reasoning models that emit whitespace chunks during thinking.
                        if let Some(text) = event
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("content"))
                            .and_then(|t| t.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            full_text.push_str(text);
                            if !text.chars().all(|c| c.is_ascii_whitespace()) {
                                on_token(text);
                                if abort.load(Ordering::Relaxed) {
                                    tracing::warn!(
                                        "stream_openai_with_tools: repetition abort after {} chars",
                                        full_text.len()
                                    );
                                    break 'stream;
                                }
                            }
                        }

                        // Tool call deltas
                        if let Some(tc_array) = event
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("tool_calls"))
                            .and_then(|t| t.as_array())
                        {
                            for tc in tc_array {
                                let idx =
                                    tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;

                                // Grow the pending list if needed
                                while pending_tool_calls.len() <= idx {
                                    pending_tool_calls.push((
                                        String::new(),
                                        String::new(),
                                        String::new(),
                                    ));
                                }

                                // First chunk for this index carries id and function.name
                                if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                    pending_tool_calls[idx].0 = id.to_string();
                                }
                                if let Some(name) = tc
                                    .get("function")
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str())
                                {
                                    pending_tool_calls[idx].1 = name.to_string();
                                    tracing::debug!(
                                        tool_index = idx,
                                        tool_name = %name,
                                        "OpenAI tool call started"
                                    );
                                }
                                // Incremental argument string
                                if let Some(args) = tc
                                    .get("function")
                                    .and_then(|f| f.get("arguments"))
                                    .and_then(|a| a.as_str())
                                {
                                    pending_tool_calls[idx].2.push_str(args);
                                }
                            }
                        }

                        // Usage chunk (sent at end with stream_options.include_usage)
                        if let Some(usage) = event.get("usage") {
                            tokens_in = usage
                                .get("prompt_tokens")
                                .and_then(|t| t.as_u64())
                                .map(|t| t as u32);
                            tokens_out = usage
                                .get("completion_tokens")
                                .and_then(|t| t.as_u64())
                                .map(|t| t as u32);
                        }
                    }
                }
            }
        }

        if abort.load(Ordering::Relaxed) {
            return Err(format!(
                "stream_aborted:repetition_detected:{} chars accumulated",
                full_text.len()
            ));
        }

        // Finalize any pending tool calls
        let tool_calls: Vec<ToolCall> = pending_tool_calls
            .into_iter()
            .filter(|(id, name, _)| !id.is_empty() && !name.is_empty())
            .map(|(id, name, args_buf)| {
                let input: serde_json::Value = serde_json::from_str(&args_buf)
                    .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                tracing::debug!(
                    tool_id = %id,
                    tool_name = %name,
                    input = %input,
                    "OpenAI tool call finalized"
                );
                ToolCall { id, name, input }
            })
            .collect();

        tracing::debug!(
            tokens_in = ?tokens_in,
            tokens_out = ?tokens_out,
            tool_calls = tool_calls.len(),
            "OpenAI tool-use streaming complete, {} text chars",
            full_text.len()
        );

        if !tool_calls.is_empty() {
            Ok(LlmTurn::ToolUse {
                calls: tool_calls,
                partial_text: full_text,
                tokens_in,
                tokens_out,
            })
        } else {
            Ok(LlmTurn::Text(LlmResponse {
                text: full_text,
                tokens_in,
                tokens_out,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LlmClient;

    #[test]
    fn openai_reasoning_models_use_high_reasoning_effort_by_default() {
        let body = LlmClient::with_budget("openai", "gpt-5.2-2025-12-11", "test-key", 12_000)
            .build_openai_prompt_body("Prove it", false);

        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["max_completion_tokens"], 12_000);
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn openrouter_payload_uses_reasoning_object() {
        let body = LlmClient::with_budget(
            "openrouter",
            "openai/gpt-5-nano",
            "test-key",
            8_000,
        )
        .build_openai_prompt_body("Prove it", true);

        assert_eq!(body["reasoning"]["effort"], "high");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn gemini_payload_uses_explicit_reasoning_effort_instead_of_extra_body() {
        let body = LlmClient::with_budget("gemini", "gemini-3-flash-preview", "test-key", 8_000)
            .build_openai_prompt_body("Prove it", false);

        assert_eq!(body["reasoning_effort"], "high");
        assert!(body.get("extra_body").is_none());
    }

    #[test]
    fn chatgpt_payload_includes_high_reasoning_effort_by_default() {
        let body = LlmClient::new("chatgpt", "gpt-5.4", "test-key").chatgpt_body("Prove it");

        assert_eq!(body["reasoning"]["effort"], "high");
    }

    #[test]
    fn anthropic_reasoning_levels_change_thinking_budget() {
        let high = LlmClient::with_budget("anthropic", "claude-sonnet-4-6", "test-key", 16_000)
            .build_anthropic_prompt_body("Prove it", false);
        let low = LlmClient::with_budget("anthropic", "claude-sonnet-4-6", "test-key", 16_000)
            .with_reasoning_level("low")
            .build_anthropic_prompt_body("Prove it", false);

        assert_eq!(high["thinking"]["type"], "enabled");
        assert_eq!(low["thinking"]["type"], "enabled");
        assert!(
            high["thinking"]["budget_tokens"]
                .as_u64()
                .expect("high budget")
                > low["thinking"]["budget_tokens"]
                    .as_u64()
                    .expect("low budget")
        );
    }
}
