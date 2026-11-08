//! Specifies the LLM adapters (#95): the requests they build for OpenAI-compatible servers and
//! Anthropic's Messages API, how they read answers and errors, and one real HTTP round trip
//! against a local mock server. No request leaves the machine.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::{
    anthropic_messages::{ANTHROPIC_VERSION, AnthropicMessagesEngine},
    base64::encode_base64,
    engine_for,
    openai_compatible::OpenAiCompatibleEngine,
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
        Some(Zeroizing::new("sk-test".to_owned())),
        Arc::new(transport.clone()),
    );

    let answer = engine.complete(&request_with_image());

    assert_eq!(answer.map(|answer| answer.text), Ok("Notes".to_owned()));
    let (url, headers, body) = transport.only_request();
    assert_eq!(url, "https://api.mistral.ai/v1/chat/completions");
    assert_eq!(header(&headers, "authorization"), Some("Bearer sk-test"));
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
        None,
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
fn an_anthropic_request_uses_the_messages_api_with_images_before_the_text() {
    let transport = RecordingTransport::answering(
        200,
        json!({
            "content": [
                {"type": "thinking", "thinking": ""},
                {"type": "text", "text": "Part one. "},
                {"type": "text", "text": "Part two."}
            ],
            "stop_reason": "end_turn"
        }),
    );
    let engine = AnthropicMessagesEngine::new(
        &connection(
            LlmKind::Anthropic,
            "https://api.anthropic.com",
            "claude-opus-5-5",
        ),
        Zeroizing::new("ak-test".to_owned()),
        Arc::new(transport.clone()),
    );

    let answer = engine.complete(&request_with_image());

    assert_eq!(
        answer.map(|answer| answer.text),
        Ok("Part one. Part two.".to_owned())
    );
    let (url, headers, body) = transport.only_request();
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
    assert_eq!(header(&headers, "x-api-key"), Some("ak-test"));
    assert_eq!(
        header(&headers, "anthropic-version"),
        Some(ANTHROPIC_VERSION)
    );
    assert_eq!(header(&headers, "authorization"), None);
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["max_tokens"], 4_000);
    assert_eq!(body["system"], "Summarize.");
    assert_eq!(
        body["messages"][0]["content"],
        json!([
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AQID"}},
            {"type": "text", "text": "Transcript"}
        ])
    );
    assert_eq!(
        header(&headers, "anthropic-beta"),
        Some("server-side-fallback-2026-07-01"),
        "a declined request is retried on a fallback model"
    );
    assert_eq!(body["fallbacks"], "default");
    assert!(body.get("temperature").is_none());
}

#[test]
fn an_anthropic_refusal_or_length_stop_is_reported() {
    let refusing =
        RecordingTransport::answering(200, json!({"content": [], "stop_reason": "refusal"}));
    let engine = AnthropicMessagesEngine::new(
        &connection(
            LlmKind::Anthropic,
            "https://api.anthropic.com/v1",
            "claude-haiku-5-5",
        ),
        Zeroizing::new("k".to_owned()),
        Arc::new(refusing.clone()),
    );
    assert_eq!(
        engine.complete(&request_with_image()),
        Err(LlmError::Refused)
    );
    let (url, headers, body) = refusing.only_request();
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
    assert_eq!(
        header(&headers, "anthropic-beta"),
        None,
        "no fallback for this model"
    );
    assert!(body.get("fallbacks").is_none());

    let cut = RecordingTransport::answering(
        200,
        json!({"content": [{"type": "text", "text": "Half"}], "stop_reason": "max_tokens"}),
    );
    let engine = AnthropicMessagesEngine::new(
        &connection(
            LlmKind::Anthropic,
            "https://api.anthropic.com",
            "claude-haiku-5-5",
        ),
        Zeroizing::new("k".to_owned()),
        Arc::new(cut),
    );
    assert_eq!(
        engine
            .complete(&request_with_image())
            .map(|answer| answer.truncated),
        Ok(true)
    );
}

#[test]
fn http_errors_map_to_portable_errors_without_their_body() {
    let cases = [
        (401, LlmError::Unauthorized),
        (403, LlmError::Unauthorized),
        (429, LlmError::RateLimited),
        (529, LlmError::RateLimited),
        (404, LlmError::Rejected { status: 404 }),
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
            Some(Zeroizing::new("k".to_owned())),
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
        None,
        Arc::new(empty),
    );
    assert_eq!(
        engine.complete(&request_with_image()),
        Err(LlmError::InvalidResponse)
    );
}

#[test]
fn an_anthropic_connection_without_a_key_is_refused_before_anything_is_sent() {
    let anthropic = connection(
        LlmKind::Anthropic,
        "https://api.anthropic.com",
        "claude-opus-5-5",
    );
    assert!(matches!(
        engine_for(&anthropic, None),
        Err(LlmError::MissingKey)
    ));
    let local = connection(
        LlmKind::OpenAiCompatible,
        "http://127.0.0.1:8080/v1",
        "local",
    );
    assert!(engine_for(&local, None).is_ok());
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
    let engine = OpenAiCompatibleEngine::new(
        &local,
        Some(Zeroizing::new("lan-key".to_owned())),
        Arc::new(transport),
    );

    let answer = std::thread::spawn(move || engine.complete(&request_with_image()))
        .join()
        .unwrap_or_else(|_| panic!("worker panicked"));

    assert_eq!(
        answer.map(|answer| answer.text),
        Ok("From the LAN".to_owned())
    );
    let received = received.lock().ok().and_then(|slot| slot.clone());
    let (authorization, body) = received.unwrap_or_else(|| panic!("the server saw the request"));
    assert_eq!(authorization.as_deref(), Some("Bearer lan-key"));
    assert_eq!(body["model"], "llama");
}
