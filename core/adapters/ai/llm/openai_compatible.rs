//! Talks to a server that speaks OpenAI's Chat Completions: one the owner runs (Ollama, llama.cpp,
//! LM Studio, vLLM), or an online service with an API key (#138: Gemini, DeepSeek, Qwen, Kimi,
//! Mistral, OpenRouter). The key goes as a Bearer header over HTTPS (or to this computer) only.

use std::sync::Arc;

use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::{
    base64::encode_base64,
    endpoint_url, status_error,
    transport::{LlmHttpResponse, LlmHttpTransport},
};
use crate::{
    domain::{
        errors::LlmError,
        llm::{LlmCompletion, LlmConnection, LlmRequest, LlmRole},
    },
    ports::ai::LlmEngine,
};

/// An engine for one OpenAI-compatible connection.
pub struct OpenAiCompatibleEngine {
    url: String,
    model: String,
    transport: Arc<dyn LlmHttpTransport>,
    /// The API key, sent only where `may_carry_key` allows.
    api_key: Option<Zeroizing<String>>,
}

/// Whether a key may go to `url`: HTTPS, or plain HTTP to this computer only, never in clear
/// across a network.
fn may_carry_key(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host
        .strip_prefix('[')
        .and_then(|bracketed| bracketed.split(']').next())
        .unwrap_or_else(|| host.rsplit_once(':').map_or(host, |(name, _)| name));
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

impl OpenAiCompatibleEngine {
    /// Creates the engine.
    ///
    /// Inputs: The connection and the transport. Outputs: The engine. Side effects: None.
    pub fn new(connection: &LlmConnection, transport: Arc<dyn LlmHttpTransport>) -> Self {
        Self {
            url: endpoint_url(connection.base_url(), "/chat/completions"),
            model: connection.model().to_owned(),
            transport,
            api_key: None,
        }
    }

    /// Adds the connection's API key (#138); a blank one is ignored.
    ///
    /// Inputs: The key. Outputs: The engine. Side effects: None; the key is never logged.
    pub fn with_api_key(mut self, api_key: Option<Zeroizing<String>>) -> Self {
        self.api_key = api_key.filter(|key| !key.trim().is_empty());
        self
    }

    /// Builds the request body.
    ///
    /// Inputs: The request. Outputs: The JSON body; the first user turn carries the images as
    /// data URLs, and turns without images are plain strings, which every server reads.
    /// Side effects: None.
    fn body(&self, request: &LlmRequest) -> Value {
        let mut messages = Vec::with_capacity(request.turns.len() + 1);
        if !request.system.trim().is_empty() {
            messages.push(json!({"role": "system", "content": request.system}));
        }
        let mut images_sent = false;
        for turn in &request.turns {
            let role = match turn.role {
                LlmRole::User => "user",
                LlmRole::Assistant => "assistant",
            };
            if turn.role == LlmRole::User && !images_sent && !request.images.is_empty() {
                images_sent = true;
                let mut content = vec![json!({"type": "text", "text": turn.text})];
                content.extend(request.images.iter().map(|image| {
                    json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!(
                                "data:{};base64,{}",
                                image.media_type(),
                                encode_base64(image.bytes())
                            )
                        }
                    })
                }));
                messages.push(json!({"role": role, "content": content}));
            } else {
                messages.push(json!({"role": role, "content": turn.text}));
            }
        }
        // No length limit is sent: servers differ on `max_tokens` and the newer field.
        json!({"model": self.model, "messages": messages, "stream": false})
    }
}

impl LlmEngine for OpenAiCompatibleEngine {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        if request.turns.is_empty() {
            return Err(LlmError::InvalidRequest {
                reason: "the conversation is empty",
            });
        }
        let mut headers = Vec::new();
        if let Some(key) = self.api_key.as_ref().filter(|_| may_carry_key(&self.url)) {
            headers.push(("authorization", format!("Bearer {}", key.as_str())));
        }
        let response = self
            .transport
            .post_json(&self.url, &headers, &self.body(request))?;
        read_answer(&response)
    }
}

/// Reads the first choice of a successful reply.
///
/// Inputs: The reply. Outputs: The answer without any leading `<think>` block that local
/// reasoning models write; `Refused` on a content filter; `InvalidResponse` when there is no
/// text. Side effects: None.
fn read_answer(response: &LlmHttpResponse) -> Result<LlmCompletion, LlmError> {
    if !(200..300).contains(&response.status) {
        return Err(status_error(response.status, &response.body));
    }
    let reply: Value =
        serde_json::from_str(&response.body).map_err(|_| LlmError::InvalidResponse)?;
    let choice = reply
        .get("choices")
        .and_then(|choices| choices.get(0))
        .ok_or(LlmError::InvalidResponse)?;
    let finish_reason = choice.get("finish_reason").and_then(Value::as_str);
    if finish_reason == Some("content_filter") {
        return Err(LlmError::Refused);
    }
    let text = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(without_reasoning)
        .unwrap_or_default();
    if text.is_empty() {
        return Err(LlmError::InvalidResponse);
    }
    Ok(LlmCompletion {
        text,
        truncated: finish_reason == Some("length"),
    })
}

/// Drops a leading `<think>…</think>` block and surrounding blank space.
///
/// Inputs: The answer text. Outputs: The visible answer. Side effects: None.
fn without_reasoning(text: &str) -> String {
    let trimmed = text.trim_start();
    let visible = trimmed
        .strip_prefix("<think>")
        .and_then(|rest| rest.split_once("</think>"))
        .map_or(trimmed, |(_, after)| after);
    visible.trim().to_owned()
}
