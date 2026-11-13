//! Specifies the LLM adapters (#95): the requests they build for OpenAI-compatible servers, how
//! they read answers and errors, that no API key is ever sent, and real HTTP round trips against a
//! local mock server. No request leaves the machine.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{
    base64::encode_base64,
    engine_for,
    openai_compatible::OpenAiCompatibleEngine,
    provider_reason,
    transport::{LlmHttpResponse, LlmHttpTransport, ReqwestLlmTransport},
};
use crate::{
    domain::{
        errors::LlmError,
        llm::{LlmConnection, LlmImage, LlmKind, LlmRequest, LlmRole, LlmTurn},
    },
    ports::ai::LlmEngine,
};

/// One captured request: URL, headers and JSON body.
type Captured = (String, Vec<(String, String)>, Value);
/// What the local server saw: the authorization header and the body.
type Seen = Option<(Option<String>, Value)>;

/// Answers with canned responses and keeps what it was sent.
#[derive(Clone, Default)]
struct RecordingTransport {
    responses: Arc<Mutex<Vec<LlmHttpResponse>>>,
    sent: Arc<Mutex<Vec<Captured>>>,
}

impl RecordingTransport {
    fn answering(status: u16, body: Value) -> Self {
        let transport = Self::default();
        if let Ok(mut responses) = transport.responses.lock() {
            responses.push(LlmHttpResponse {
                status,
                body: body.to_string(),
            });
        }
        transport
    }

    fn only_request(&self) -> Captured {
        let sent = self
            .sent
            .lock()
            .map(|sent| sent.clone())
            .unwrap_or_default();
        assert_eq!(sent.len(), 1, "one request");
        sent.into_iter().next().unwrap_or_default()
    }
}

impl LlmHttpTransport for RecordingTransport {
    fn post_json(
        &self,
        url: &str,
        headers: &[(&'static str, String)],
        body: &Value,
    ) -> Result<LlmHttpResponse, LlmError> {
        if let Ok(mut sent) = self.sent.lock() {
            sent.push((
                url.to_owned(),
                headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), value.clone()))
                    .collect(),
                body.clone(),
            ));
        }
        self.responses
            .lock()
            .ok()
            .and_then(|mut responses| responses.pop())
            .ok_or(LlmError::Unreachable)
    }
}

fn connection(kind: LlmKind, base_url: &str, model: &str) -> LlmConnection {
    LlmConnection::with_id("c1", "Test", kind, base_url, model, 8_192)
        .unwrap_or_else(|error| panic!("valid connection: {error}"))
}

fn request_with_image() -> LlmRequest {
    LlmRequest {
        system: "Summarize.".to_owned(),
        turns: vec![LlmTurn {
            role: LlmRole::User,
            text: "Transcript".to_owned(),
        }],
        images: LlmImage::new("image/png", vec![1, 2, 3])
            .into_iter()
            .collect(),
        max_output_tokens: 4_000,
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[test]
fn base64_matches_the_standard_vectors() {
    assert_eq!(encode_base64(b""), "");
    assert_eq!(encode_base64(b"f"), "Zg==");
    assert_eq!(encode_base64(b"fo"), "Zm8=");
    assert_eq!(encode_base64(b"foo"), "Zm9v");
    assert_eq!(encode_base64(b"foobar"), "Zm9vYmFy");
    assert_eq!(encode_base64(&[0xfb, 0xff]), "+/8=");
}

#[test]
fn an_openai_compatible_request_carries_system_text_and_data_url_images() {
    let transport = RecordingTransport::answering(
        200,
        json!({"choices": [{"message": {"content": "Notes"}, "finish_reason": "stop"}]}),
    );
    let engine = OpenAiCompatibleEngine::new(
        &connection(
            LlmKind::OpenAiCompatible,
            "https://api.mistral.ai/v1",
            "mistral-large",
        ),
        Arc::new(transport.clone()),
    );

    let answer = engine.complete(&request_with_image());

    assert_eq!(answer.map(|answer| answer.text), Ok("Notes".to_owned()));
    let (url, headers, body) = transport.only_request();
    assert_eq!(url, "https://api.mistral.ai/v1/chat/completions");
    assert_eq!(
        header(&headers, "authorization"),
        None,
        "no API key is sent"
    );
    assert_eq!(body["model"], "mistral-large");
    assert_eq!(body["stream"], false);
    assert_eq!(
        body["messages"][0],
        json!({"role": "system", "content": "Summarize."})
    );
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(
        body["messages"][1]["content"],
        json!([
            {"type": "text", "text": "Transcript"},
            {"type": "image_url", "image_url": {"url": "data:image/png;base64,AQID"}}
        ])
    );
}

#[test]
fn a_local_server_needs_no_key_takes_plain_text_and_its_path_is_completed() {
    let transport = RecordingTransport::answering(
        200,
        json!({"choices": [{"message": {"content": "<think>hmm</think>\n\nShort notes"}, "finish_reason": "length"}]}),
    );
    let engine = OpenAiCompatibleEngine::new(
        &connection(
            LlmKind::OpenAiCompatible,
            "http://192.168.1.20:11434",
            "llama3.1:8b",
        ),
        Arc::new(transport.clone()),
    );
    let mut request = request_with_image();
    request.images.clear();
    request.turns.push(LlmTurn {
        role: LlmRole::Assistant,
        text: "Earlier answer".to_owned(),
    });
    request.turns.push(LlmTurn {
        role: LlmRole::User,
        text: "And then?".to_owned(),
    });

    let answer = engine.complete(&request);

    let answer = answer.unwrap_or_else(|error| panic!("answer: {error}"));
    assert_eq!(answer.text, "Short notes", "a reasoning block is dropped");
    assert!(answer.truncated);
    let (url, headers, body) = transport.only_request();
    assert_eq!(url, "http://192.168.1.20:11434/v1/chat/completions");
    assert_eq!(header(&headers, "authorization"), None);
    assert_eq!(
        body["messages"][1],
        json!({"role": "user", "content": "Transcript"})
    );
    assert_eq!(body["messages"][2]["role"], "assistant");
    assert_eq!(body["messages"][3]["content"], "And then?");
}

#[test]
fn http_errors_map_to_portable_errors_keeping_the_reason_out_of_their_text() {
    let cases = [
        (401, LlmError::Unauthorized),
        (403, LlmError::Unauthorized),
        (429, LlmError::RateLimited),
        (529, LlmError::RateLimited),
        (
            404,
            LlmError::Rejected {
                status: 404,
                provider_reason: Some("secret meeting text".to_owned()),
            },
        ),
        (500, LlmError::ServerFailed { status: 500 }),
    ];
    for (status, expected) in cases {
        let transport = RecordingTransport::answering(
            status,
            json!({"error": {"message": "secret meeting text"}}),
        );
        let engine = OpenAiCompatibleEngine::new(
            &connection(
                LlmKind::OpenAiCompatible,
                "https://api.openai.com/v1",
                "gpt-5-mini",
            ),
            Arc::new(transport),
        );
        let error = engine.complete(&request_with_image());
        assert_eq!(error, Err(expected.clone()), "HTTP {status}");
        assert!(!format!("{expected}").contains("secret"));
    }
    let empty = RecordingTransport::answering(200, json!({"choices": []}));
    let engine = OpenAiCompatibleEngine::new(
        &connection(
            LlmKind::OpenAiCompatible,
            "https://api.openai.com/v1",
            "gpt-5-mini",
        ),
        Arc::new(empty),
    );
    assert_eq!(
        engine.complete(&request_with_image()),
        Err(LlmError::InvalidResponse)
    );
}

#[test]
fn a_provider_reason_is_one_short_line_or_nothing() {
    assert_eq!(provider_reason("<html>Bad gateway</html>"), None);
    assert_eq!(provider_reason(r#"{"error": {}}"#), None);
    assert_eq!(
        provider_reason(r#"{"error": "model 'x' not found"}"#).as_deref(),
        Some("model 'x' not found")
    );
    assert_eq!(
        provider_reason(r#"{"error": {"type": "bad", "message": "line one\n\tline\u0007 two"}}"#)
            .as_deref(),
        Some("bad: line one line two")
    );
    let long = format!(r#"{{"error": {{"message": "{}"}}}}"#, "é".repeat(500));
    let reason = provider_reason(&long).unwrap_or_default();
    assert_eq!(reason.chars().count(), 300);
    assert!(reason.ends_with('…'));
}

#[test]
fn the_reqwest_transport_round_trips_with_a_local_server() {
    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let received: Arc<Mutex<Seen>> = Arc::default();
    let seen = Arc::clone(&received);
    let address = runtime.block_on(async move {
        let router = axum::Router::new().route(
            "/v1/chat/completions",
            axum::routing::post(
                move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                    let seen = Arc::clone(&seen);
                    async move {
                        let authorization = headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_owned);
                        if let Ok(mut slot) = seen.lock() {
                            *slot = Some((authorization, body));
                        }
                        axum::Json(json!({
                            "choices": [{"message": {"content": "From the LAN"}, "finish_reason": "stop"}]
                        }))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("bind: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("address: {error}"));
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        address
    });
    let local = connection(
        LlmKind::OpenAiCompatible,
        &format!("http://{address}/v1"),
        "llama",
    );
    let transport = ReqwestLlmTransport::new().unwrap_or_else(|error| panic!("client: {error}"));
    let engine = OpenAiCompatibleEngine::new(&local, Arc::new(transport));

    let answer = std::thread::spawn(move || engine.complete(&request_with_image()))
        .join()
        .unwrap_or_else(|_| panic!("worker panicked"));

    assert_eq!(
        answer.map(|answer| answer.text),
        Ok("From the LAN".to_owned())
    );
    let received = received.lock().ok().and_then(|slot| slot.clone());
    let (authorization, body) = received.unwrap_or_else(|| panic!("the server saw the request"));
    assert_eq!(authorization, None, "no API key is sent");
    assert_eq!(body["model"], "llama");
}

#[test]
fn the_reqwest_transport_reads_an_error_reply_for_its_reason() {
    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|error| panic!("runtime: {error}"));
    let address = runtime.block_on(async move {
        let router = axum::Router::new().route(
            "/v1/chat/completions",
            axum::routing::post(|| async {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    axum::Json(json!({"type": "error", "error": {
                        "type": "invalid_request_error", "message": "max_tokens: too large"
                    }})),
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("bind: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("address: {error}"));
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        address
    });
    let local = connection(
        LlmKind::OpenAiCompatible,
        &format!("http://{address}/v1"),
        "llama",
    );
    let transport = ReqwestLlmTransport::new().unwrap_or_else(|error| panic!("client: {error}"));
    let engine = OpenAiCompatibleEngine::new(&local, Arc::new(transport));

    let answer = std::thread::spawn(move || engine.complete(&request_with_image()))
        .join()
        .unwrap_or_else(|_| panic!("worker panicked"));

    assert_eq!(
        answer.map(|answer| answer.text),
        Err(LlmError::Rejected {
            status: 400,
            provider_reason: Some("invalid_request_error: max_tokens: too large".to_owned()),
        })
    );
}

#[test]
fn every_server_connection_builds_without_a_key() {
    let local = connection(
        LlmKind::OpenAiCompatible,
        "http://127.0.0.1:8080/v1",
        "local",
    );
    assert!(engine_for(&local).is_ok());
}
