//! Language model adapters (#95): one for OpenAI-compatible Chat Completions servers (OpenAI,
//! Mistral, and local servers such as Ollama, llama.cpp, LM Studio and vLLM) and one for
//! Anthropic's Messages API, both behind the `LlmEngine` port.

/// Anthropic's Messages API.
pub mod anthropic_messages;
/// Standard base64, for images sent inline.
pub mod base64;
/// OpenAI-compatible Chat Completions.
pub mod openai_compatible;
/// The blocking HTTP client both adapters post through.
pub mod transport;

#[cfg(test)]
mod llm_adapters_tests;

use std::sync::Arc;

use zeroize::Zeroizing;

use crate::{
    domain::{
        errors::LlmError,
        llm::{LlmConnection, LlmKind},
    },
    ports::ai::LlmEngine,
};

/// Builds the engine for a connection, posting through reqwest.
///
/// Inputs: The connection and its API key, if any. Outputs: The engine; `MissingKey` for an
/// Anthropic connection without a key, `Unreachable` when no HTTP client can be built.
/// Side effects: None until the engine is used.
pub fn engine_for(
    connection: &LlmConnection,
    api_key: Option<Zeroizing<String>>,
) -> Result<Box<dyn LlmEngine>, LlmError> {
    let transport = Arc::new(transport::ReqwestLlmTransport::new()?);
    Ok(match connection.kind() {
        LlmKind::OpenAiCompatible => Box::new(openai_compatible::OpenAiCompatibleEngine::new(
            connection, api_key, transport,
        )),
        LlmKind::Anthropic => Box::new(anthropic_messages::AnthropicMessagesEngine::new(
            connection,
            api_key.ok_or(LlmError::MissingKey)?,
            transport,
        )),
    })
}

/// The most characters of a provider's reason that are kept.
const MAX_REASON_CHARS: usize = 300;

/// Maps an HTTP status other than success to a portable error.
///
/// Inputs: The status and the error reply's body. Outputs: The error; a rejection keeps the
/// server's reason (`provider_reason`), everything else drops the body. Side effects: None.
pub(crate) fn status_error(status: u16, body: &str) -> LlmError {
    match status {
        401 | 403 => LlmError::Unauthorized,
        // 529 is Anthropic's "overloaded".
        429 | 529 => LlmError::RateLimited,
        400..=499 => LlmError::Rejected {
            status,
            provider_reason: provider_reason(body),
        },
        _ => LlmError::ServerFailed { status },
    }
}

/// Reads the reason from an error reply, in the shape Anthropic and OpenAI-compatible servers
/// share: `{"error": {"type", "message"}}`, or `{"error": "text"}` (Ollama).
///
/// Inputs: The reply body. Outputs: `"<type>: <message>"` (or just one of them) on one line,
/// control characters and runs of spaces collapsed, cut to 300 characters with `…`; `None` when
/// the body is not such JSON. Side effects: None.
pub(crate) fn provider_reason(body: &str) -> Option<String> {
    let reply: serde_json::Value = serde_json::from_str(body).ok()?;
    let error = reply.get("error")?;
    let text_of = |name: &str| {
        error
            .get(name)
            .and_then(serde_json::Value::as_str)
            .filter(|text| !text.trim().is_empty())
    };
    let joined = match (error.as_str(), text_of("type"), text_of("message")) {
        (Some(text), _, _) => text.to_owned(),
        (None, Some(kind), Some(message)) => format!("{kind}: {message}"),
        (None, Some(only), None) | (None, None, Some(only)) => only.to_owned(),
        (None, None, None) => return None,
    };
    let one_line = joined
        .split(|character: char| character.is_whitespace() || character.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if one_line.is_empty() {
        return None;
    }
    if one_line.chars().count() <= MAX_REASON_CHARS {
        return Some(one_line);
    }
    let mut cut: String = one_line.chars().take(MAX_REASON_CHARS - 1).collect();
    cut.push('…');
    Some(cut)
}

/// Joins an endpoint path to a base URL, adding `/v1` when the base has no path.
///
/// Inputs: The checked base URL (no trailing slash) and the path after the version, such as
/// `/chat/completions`. Outputs: The full URL. Side effects: None.
pub(crate) fn endpoint_url(base_url: &str, path: &str) -> String {
    let after_scheme = base_url
        .split_once("://")
        .map_or(base_url, |(_, rest)| rest);
    let has_path = after_scheme.contains('/');
    if has_path {
        format!("{base_url}{path}")
    } else {
        format!("{base_url}/v1{path}")
    }
}
