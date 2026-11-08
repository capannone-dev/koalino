//! Talks to any server that speaks OpenAI's Chat Completions: OpenAI, Mistral, and local
//! servers such as Ollama, llama.cpp, LM Studio and vLLM.

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
    api_key: Option<Zeroizing<String>>,
    transport: Arc<dyn LlmHttpTransport>,
}

impl OpenAiCompatibleEngine {
    /// Creates the engine.
    ///
    /// Inputs: The connection, its API key (local servers often need none) and the transport.
    /// Outputs: The engine. Side effects: None.
    pub fn new(
        connection: &LlmConnection,
        api_key: Option<Zeroizing<String>>,
        transport: Arc<dyn LlmHttpTransport>,
    ) -> Self {
        Self {
            url: endpoint_url(connection.base_url(), "/chat/completions"),
            model: connection.model().to_owned(),
            api_key: api_key.filter(|key| !key.trim().is_empty()),
            transport,
        }
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
        // No length limit is sent: OpenAI's reasoning models refuse `max_tokens`, and servers
        // differ on the newer field.
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
        if let Some(key) = &self.api_key {
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
        return Err(status_error(response.status));
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
