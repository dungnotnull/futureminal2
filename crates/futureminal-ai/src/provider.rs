//! AI provider abstraction layer with real HTTP + SSE streaming.

use async_trait::async_trait;
use futures_util::Stream;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;

/// Errors that can occur during AI provider operations.
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Provider not configured: {0}")]
    NotConfigured(String),
    #[error("Streaming error: {0}")]
    Stream(String),
    #[error("Rate limited. Retry after {0}s")]
    RateLimited(u64),
    #[error("Timeout")]
    Timeout,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Request to generate a completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

/// Response from a completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: Option<u32>,
    pub finish_reason: Option<String>,
}

/// Streaming chunk from a completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub content: String,
    pub finish_reason: Option<String>,
}

/// Trait implemented by all AI backends.
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn is_available(&self) -> bool;
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError>;
    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>;
}

// ?? Anthropic Provider ??

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::new(),
            base_url: "https://api.anthropic.com".into(),
        }
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }

    async fn is_available(&self) -> bool { !self.api_key.is_empty() }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        let body = build_anthropic_body(&self.model, &request);
        let resp = self.client.post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send().await.map_err(ProviderError::Http)?;
        let status = resp.status();
        let text = resp.text().await.map_err(ProviderError::Http)?;
        if !status.is_success() {
            return Err(ProviderError::Api { status: status.as_u16(), message: text });
        }
        let json: serde_json::Value = serde_json::from_str(&text)?;
        let content = json["content"][0]["text"].as_str().unwrap_or("").to_string();
        Ok(CompletionResponse {
            content,
            model: self.model.clone(),
            tokens_used: json["usage"]["input_tokens"].as_u64().map(|v| v as u32),
            finish_reason: json["stop_reason"].as_str().map(|s| s.to_string()),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError> {
        let body = build_anthropic_body(&self.model, &request);
        let resp = self.client.post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Accept", "text/event-stream")
            .json(&body)
            .send().await.map_err(ProviderError::Http)?;

        Ok(Box::pin(parse_sse_stream(resp.bytes_stream())))
    }
}

fn build_anthropic_body(model: &str, req: &CompletionRequest) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": req.messages.iter().map(|m| serde_json::json!({
            "role": match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            "content": m.content,
        })).collect::<Vec<_>>(),
        "max_tokens": req.max_tokens.unwrap_or(4096),
        "temperature": req.temperature.unwrap_or(0.7),
        "stream": req.stream,
    })
}

// ?? OpenAI Provider ??

pub struct OpenAiProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self { api_key: api_key.into(), model: model.into(), client: reqwest::Client::new(), base_url: "https://api.openai.com".into() }
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    fn name(&self) -> &str { "openai" }
    async fn is_available(&self) -> bool { !self.api_key.is_empty() }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        let body = build_openai_body(&self.model, &request);
        let resp = self.client.post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key).json(&body).send().await.map_err(ProviderError::Http)?;
        let status = resp.status();
        let text = resp.text().await.map_err(ProviderError::Http)?;
        if !status.is_success() { return Err(ProviderError::Api { status: status.as_u16(), message: text }); }
        let json: serde_json::Value = serde_json::from_str(&text)?;
        let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        Ok(CompletionResponse { content, model: self.model.clone(), tokens_used: json["usage"]["total_tokens"].as_u64().map(|v| v as u32), finish_reason: json["choices"][0]["finish_reason"].as_str().map(|s| s.to_string()) })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError> {
        let mut body = build_openai_body(&self.model, &request);
        body["stream"] = serde_json::Value::Bool(true);
        let resp = self.client.post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.api_key).json(&body).send().await.map_err(ProviderError::Http)?;
        Ok(Box::pin(parse_sse_stream(resp.bytes_stream())))
    }
}

fn build_openai_body(model: &str, req: &CompletionRequest) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": req.messages.iter().map(|m| serde_json::json!({
            "role": match m.role { Role::System => "system", Role::User => "user", Role::Assistant => "assistant" },
            "content": m.content,
        })).collect::<Vec<_>>(),
        "max_tokens": req.max_tokens,
        "temperature": req.temperature,
        "stream": req.stream,
    })
}

// ?? Ollama Provider ??

pub struct OllamaProvider {
    url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(url: impl Into<String>, model: impl Into<String>) -> Self {
        Self { url: url.into(), model: model.into(), client: reqwest::Client::new() }
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn name(&self) -> &str { "ollama" }
    async fn is_available(&self) -> bool {
        match self.client.get(format!("{}/api/tags", self.url)).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        let prompt = request.messages.iter().map(|m| format!("{}: {}", format_role(m.role), m.content)).collect::<Vec<_>>().join("\n");
        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "options": { "temperature": request.temperature.unwrap_or(0.7), "num_predict": request.max_tokens }
        });
        let resp = self.client.post(format!("{}/api/generate", self.url)).json(&body).send().await.map_err(ProviderError::Http)?;
        let status = resp.status();
        let text = resp.text().await.map_err(ProviderError::Http)?;
        if !status.is_success() { return Err(ProviderError::Api { status: status.as_u16(), message: text }); }
        let json: serde_json::Value = serde_json::from_str(&text)?;
        Ok(CompletionResponse { content: json["response"].as_str().unwrap_or("").to_string(), model: self.model.clone(), tokens_used: json["eval_count"].as_u64().map(|v| v as u32), finish_reason: json["done"].as_bool().and_then(|d| if d { Some("stop".into()) } else { None }) })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError> {
        let prompt = request.messages.iter().map(|m| format!("{}: {}", format_role(m.role), m.content)).collect::<Vec<_>>().join("\n");
        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": true,
            "options": { "temperature": request.temperature.unwrap_or(0.7), "num_predict": request.max_tokens }
        });
        let resp = self.client.post(format!("{}/api/generate", self.url)).json(&body).send().await.map_err(ProviderError::Http)?;
        Ok(Box::pin(parse_ndjson_stream(resp.bytes_stream())))
    }
}

// ?? Generic OpenAI-Compatible Provider ??

pub struct OpenAiCompatibleProvider {
    url: String,
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(url: impl Into<String>, api_key: Option<impl Into<String>>, model: impl Into<String>) -> Self {
        Self { url: url.into(), api_key: api_key.map(|k| k.into()), model: model.into(), client: reqwest::Client::new() }
    }
}

#[async_trait]
impl AiProvider for OpenAiCompatibleProvider {
    fn name(&self) -> &str { "openai-compatible" }
    async fn is_available(&self) -> bool {
        match self.client.get(format!("{}/models", self.url)).send().await { Ok(r) => r.status().is_success(), Err(_) => false }
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, ProviderError> {
        let mut req = self.client.post(format!("{}/v1/chat/completions", self.url)).json(&request);
        if let Some(ref key) = self.api_key { req = req.bearer_auth(key); }
        let resp = req.send().await.map_err(ProviderError::Http)?;
        let status = resp.status();
        let text = resp.text().await.map_err(ProviderError::Http)?;
        if !status.is_success() { return Err(ProviderError::Api { status: status.as_u16(), message: text }); }
        let json: serde_json::Value = serde_json::from_str(&text)?;
        let content = json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
        Ok(CompletionResponse { content, model: self.model.clone(), tokens_used: None, finish_reason: None })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError> {
        let mut body = build_openai_body(&self.model, &request);
        body["stream"] = serde_json::Value::Bool(true);
        let mut req = self.client.post(format!("{}/v1/chat/completions", self.url)).json(&body);
        if let Some(ref key) = self.api_key { req = req.bearer_auth(key); }
        let resp = req.send().await.map_err(ProviderError::Http)?;
        Ok(Box::pin(parse_sse_stream(resp.bytes_stream())))
    }
}

fn format_role(role: Role) -> &'static str {
    match role { Role::System => "System", Role::User => "User", Role::Assistant => "Assistant" }
}

// ?? SSE / NDJSON Stream Parsing ??

use bytes::Bytes;

fn parse_sse_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin + 'static,
) -> impl Stream<Item = Result<StreamChunk, ProviderError>> + Send {
    futures_util::stream::unfold(
        (stream, String::new()),
        |(mut stream, mut buffer)| async move {
            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let chunk: Bytes = chunk;
                        buffer.push_str(&String::from_utf8_lossy(&chunk));
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer.drain(..=pos).collect::<String>();
                            let line = line.trim();
                            if line.starts_with("data: ") {
                                let data = &line[6..];
                                if data == "[DONE]" {
                                    return Some((Ok(StreamChunk { content: "".into(), finish_reason: Some("stop".into()) }), (stream, buffer)));
                                }
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                        return Some((Ok(StreamChunk { content: content.to_string(), finish_reason: None }), (stream, buffer)));
                                    }
                                    if let Some(content) = json["content_block"]["text"].as_str() {
                                        return Some((Ok(StreamChunk { content: content.to_string(), finish_reason: None }), (stream, buffer)));
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => return Some((Err(ProviderError::Http(e)), (stream, buffer))),
                    None => return None,
                }
            }
        },
    )
}

fn parse_ndjson_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + Unpin + 'static,
) -> impl Stream<Item = Result<StreamChunk, ProviderError>> + Send {
    futures_util::stream::unfold(
        (stream, String::new()),
        |(mut stream, mut buffer)| async move {
            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        let chunk: Bytes = chunk;
                        buffer.push_str(&String::from_utf8_lossy(&chunk));
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer.drain(..=pos).collect::<String>();
                            let line = line.trim();
                            if line.is_empty() { continue; }
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                                if let Some(content) = json["response"].as_str() {
                                    let done = json["done"].as_bool().unwrap_or(false);
                                    return Some((Ok(StreamChunk { content: content.to_string(), finish_reason: if done { Some("stop".into()) } else { None } }), (stream, buffer)));
                                }
                            }
                        }
                    }
                    Some(Err(e)) => return Some((Err(ProviderError::Http(e)), (stream, buffer))),
                    None => return None,
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_request_serde() {
        let req = CompletionRequest {
            messages: vec![ChatMessage { role: Role::User, content: "Hello".into() }],
            model: "gpt-4o".into(),
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_provider_names() {
        assert_eq!(AnthropicProvider::new("k", "claude").name(), "anthropic");
        assert_eq!(OpenAiProvider::new("k", "gpt-4o").name(), "openai");
        assert_eq!(OllamaProvider::new("http://localhost:11434", "llama3").name(), "ollama");
    }
}

