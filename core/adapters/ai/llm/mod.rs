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

/// Maps an HTTP status other than success to a portable error.
///
/// Inputs: The status. Outputs: The error. Side effects: None.
pub(crate) fn status_error(status: u16) -> LlmError {
    match status {
        401 | 403 => LlmError::Unauthorized,
        // 529 is Anthropic's "overloaded".
        429 | 529 => LlmError::RateLimited,
        400..=499 => LlmError::Rejected { status },
        _ => LlmError::ServerFailed { status },
    }
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
