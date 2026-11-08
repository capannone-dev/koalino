//! Talks to Anthropic's Messages API (`POST /v1/messages`), which reads images natively.

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

/// The API version sent in `anthropic-version`.
pub const ANTHROPIC_VERSION: &str = "2026-10-21";
/// The beta that lets the API retry a declined request on a fallback model.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Models that take the server-side fallback on Anthropic's own API.
const MODELS_WITH_FALLBACK: [&str; 4] = [
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];
/// Anthropic's own API host; proxies may not know the fallback beta.
const ANTHROPIC_HOST_URL: &str = "https://api.anthropic.com";

/// An engine for one Anthropic connection.
pub struct AnthropicMessagesEngine {
    url: String,
    model: String,
    api_key: Zeroizing<String>,
    with_fallback: bool,
    transport: Arc<dyn LlmHttpTransport>,
}

impl AnthropicMessagesEngine {
    /// Creates the engine.
    ///
    /// Inputs: The connection, its API key and the transport. Outputs: The engine.
    /// Side effects: None.
    pub fn new(
        connection: &LlmConnection,
        api_key: Zeroizing<String>,
        transport: Arc<dyn LlmHttpTransport>,
    ) -> Self {
        let base_url = connection.base_url();
        Self {
            url: endpoint_url(base_url, "/messages"),
            model: connection.model().to_owned(),
            api_key,
            with_fallback: (base_url == ANTHROPIC_HOST_URL
                || base_url == format!("{ANTHROPIC_HOST_URL}/v1"))
                && MODELS_WITH_FALLBACK.contains(&connection.model()),
            transport,
        }
    }

    /// Builds the request body.
    ///
    /// Inputs: The request. Outputs: The JSON body; the first user turn carries the images as
    /// base64 blocks placed before its text. Side effects: None.
    fn body(&self, request: &LlmRequest) -> Value {
        let mut images_sent = false;
        let messages: Vec<Value> = request
            .turns
            .iter()
            .map(|turn| {
                let role = match turn.role {
                    LlmRole::User => "user",
                    LlmRole::Assistant => "assistant",
                };
                let mut content = Vec::new();
                if turn.role == LlmRole::User && !images_sent {
                    images_sent = true;
                    content.extend(request.images.iter().map(|image| {
                        json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": image.media_type(),
                                "data": encode_base64(image.bytes())
                            }
                        })
                    }));
                }
                content.push(json!({"type": "text", "text": turn.text}));
                json!({"role": role, "content": content})
            })
            .collect();
        let mut body = json!({
            "model": self.model,
            "max_tokens": request.max_output_tokens.max(1),
            "messages": messages,
        });
        if !request.system.trim().is_empty() {
            body["system"] = json!(request.system);
        }
        if self.with_fallback {
            body["fallbacks"] = json!("default");
        }
        body
    }
}

impl LlmEngine for AnthropicMessagesEngine {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        if request.turns.is_empty() {
            return Err(LlmError::InvalidRequest {
                reason: "the conversation is empty",
            });
        }
        let mut headers = vec![
            ("x-api-key", self.api_key.as_str().to_owned()),
            ("anthropic-version", ANTHROPIC_VERSION.to_owned()),
        ];
        if self.with_fallback {
            headers.push(("anthropic-beta", FALLBACK_BETA.to_owned()));
        }
        let response = self
            .transport
            .post_json(&self.url, &headers, &self.body(request))?;
        read_answer(&response)
    }
}

/// Reads a successful reply, checking why it stopped before reading its text.
///
/// Inputs: The reply. Outputs: Its text blocks joined; `Refused` when the model declined;
/// `InvalidResponse` when there is no text. Thinking blocks are skipped. Side effects: None.
fn read_answer(response: &LlmHttpResponse) -> Result<LlmCompletion, LlmError> {
    if !(200..300).contains(&response.status) {
        return Err(status_error(response.status));
    }
    let reply: Value =
        serde_json::from_str(&response.body).map_err(|_| LlmError::InvalidResponse)?;
    let stop_reason = reply.get("stop_reason").and_then(Value::as_str);
    if stop_reason == Some("refusal") {
        return Err(LlmError::Refused);
    }
    let text: String = reply
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err(LlmError::InvalidResponse);
    }
    Ok(LlmCompletion {
        text: text.trim().to_owned(),
        truncated: stop_reason == Some("max_tokens"),
    })
}
