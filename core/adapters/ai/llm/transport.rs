//! Posts JSON to a language model server over blocking reqwest, keeping HTTP types out of the
//! adapters' callers.

use std::time::Duration;

use serde_json::Value;

use crate::domain::errors::LlmError;

/// How long one answer may take; local models on modest hardware can be slow.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(600);
/// How long reaching the server may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// A server's reply: its status and body.
#[derive(Clone, Debug, Default)]
pub struct LlmHttpResponse {
    /// The HTTP status.
    pub status: u16,
    /// The body; read only on success, since an error body can echo meeting content.
    pub body: String,
}

/// Posts one JSON request.
pub trait LlmHttpTransport: Send + Sync {
    /// Posts `body` to `url` with `headers` and waits for the reply.
    ///
    /// Inputs: The URL, headers (some hold the API key) and body. Outputs: The reply, or
    /// `Unreachable`. Side effects: One HTTP request; nothing is logged.
    fn post_json(
        &self,
        url: &str,
        headers: &[(&'static str, String)],
        body: &Value,
    ) -> Result<LlmHttpResponse, LlmError>;
}

/// Posts through reqwest's blocking client, so callers run it off the async runtime.
pub struct ReqwestLlmTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestLlmTransport {
    /// Builds a client with timeouts and no redirects, so a key is never sent to another host.
    ///
    /// Inputs: None. Outputs: The transport, or `Unreachable`. Side effects: None.
    pub fn new() -> Result<Self, LlmError> {
        reqwest::blocking::Client::builder()
            .timeout(ANSWER_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map(|client| Self { client })
            .map_err(|_| LlmError::Unreachable)
    }
}

impl LlmHttpTransport for ReqwestLlmTransport {
    fn post_json(
        &self,
        url: &str,
        headers: &[(&'static str, String)],
        body: &Value,
    ) -> Result<LlmHttpResponse, LlmError> {
        let mut request = self.client.post(url).json(body);
        for (name, value) in headers {
            request = request.header(*name, value);
        }
        let response = request.send().map_err(|_| LlmError::Unreachable)?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Ok(LlmHttpResponse {
                status,
                body: String::new(),
            });
        }
        let body = response.text().map_err(|_| LlmError::Unreachable)?;
        Ok(LlmHttpResponse { status, body })
    }
}
