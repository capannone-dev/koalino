//! Defines mock-first contracts for the OpenAI Whisper transcription adapter.

use std::sync::{Arc, Mutex};

use crate::{
    adapters::ai::openai_whisper_adapter::{
        AudioFileSource, OPENAI_MAX_AUDIO_BYTES, OpenAiHttpClient, OpenAiHttpResponse,
        OpenAiWhisperConfig, OpenAiWhisperTranscriptionEngine,
    },
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, DeviceId, MeetingId, RecordingId},
    },
    ports::ai::TranscriptionEngine,
};

type CapturedRequest = (String, String, String, Vec<u8>);

#[derive(Clone)]
struct MockHttpClient {
    responses: Arc<Mutex<Vec<OpenAiHttpResponse>>>,
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
}
impl MockHttpClient {
    fn new(responses: Vec<OpenAiHttpResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
impl OpenAiHttpClient for MockHttpClient {
    fn post_transcription(
        &self,
        authorization: &str,
        filename: &str,
        content_type: &str,
        audio_bytes: Vec<u8>,
        _model: &str,
        _language: Option<&str>,
    ) -> Result<OpenAiHttpResponse, TranscriptionError> {
        self.requests
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?
            .push((
                authorization.to_owned(),
                filename.to_owned(),
                content_type.to_owned(),
                audio_bytes,
            ));
        self.responses
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?
            .pop()
            .ok_or(TranscriptionError::Unavailable)
    }
}

struct StaticAudioSource(Vec<u8>);
impl AudioFileSource for StaticAudioSource {
    fn load_wav(&self, _: &AudioRecording) -> Result<Vec<u8>, TranscriptionError> {
        Ok(self.0.clone())
    }
}
fn recording() -> AudioRecording {
    AudioRecording::new(
        RecordingId::new("openai-test-recording"),
        MeetingId::new("openai-test-meeting"),
        DeviceId::new("openai-test-device"),
    )
}

#[test]
fn successful_response_uses_wav_filename_and_returns_portable_transcript() {
    let http = MockHttpClient::new(vec![OpenAiHttpResponse::success(
        r#"{"text":" Ciao mondo "}"#,
    )]);
    let requests = http.requests.clone();
    let engine = OpenAiWhisperTranscriptionEngine::with_dependencies(
        OpenAiWhisperConfig::new("test-secret", "whisper-1", Some("it".to_owned()))
            .unwrap_or_else(|error| panic!("config should validate: {error}")),
        Arc::new(StaticAudioSource(vec![1, 2, 3])),
        Arc::new(http),
    );
    let transcript = engine
        .transcribe(&recording())
        .unwrap_or_else(|error| panic!("mock success should transcribe: {error}"));
    assert_eq!(transcript.content(), "Ciao mondo");
    let recorded_requests = requests
        .lock()
        .unwrap_or_else(|_| panic!("mock requests should be readable"));
    assert_eq!(recorded_requests[0].1, "audio_chunk.wav");
    assert_eq!(recorded_requests[0].2, "audio/wav");
    assert_eq!(recorded_requests[0].0, "Bearer test-secret");
}

#[test]
fn rejects_payload_over_openai_limit_without_http_request() {
    let http = MockHttpClient::new(vec![]);
    let requests = http.requests.clone();
    let engine = OpenAiWhisperTranscriptionEngine::with_dependencies(
        OpenAiWhisperConfig::new("test-secret", "whisper-1", None)
            .unwrap_or_else(|error| panic!("config should validate: {error}")),
        Arc::new(StaticAudioSource(vec![0; OPENAI_MAX_AUDIO_BYTES + 1])),
        Arc::new(http),
    );
    assert!(matches!(
        engine.transcribe(&recording()),
        Err(TranscriptionError::PayloadTooLarge { .. })
    ));
    assert!(
        requests
            .lock()
            .unwrap_or_else(|_| panic!("mock requests should be readable"))
            .is_empty()
    );
}

#[test]
fn unauthorized_rate_limited_and_server_errors_are_safe_and_bounded() {
    for status in [401_u16, 429_u16, 500_u16] {
        let engine = OpenAiWhisperTranscriptionEngine::with_dependencies(
            OpenAiWhisperConfig::new("test-secret", "whisper-1", None)
                .unwrap_or_else(|error| panic!("config should validate: {error}")),
            Arc::new(StaticAudioSource(vec![1])),
            Arc::new(MockHttpClient::new(vec![OpenAiHttpResponse::failure(
                status,
            )])),
        );
        let error = engine
            .transcribe(&recording())
            .err()
            .unwrap_or_else(|| panic!("HTTP failure should be portable"));
        assert!(!error.to_string().contains("test-secret"));
    }
}

/// Requires cloud transcription to forward prior chunk text as OpenAI's multipart prompt field.
///
/// Inputs: The OpenAI adapter source, which will receive `TranscriptionContext` during the
/// context-propagation implementation.
/// Outputs: Confirms the portable context is accepted and multipart construction contains the
/// documented `prompt` field.
/// Side effects: Reads compile-time source text only; it does not make an HTTP request or expose
/// configuration secrets.
#[test]
fn openai_adapter_receives_context_previous_text_and_includes_multipart_prompt_field() {
    let adapter_source = include_str!("openai_whisper_adapter.rs");

    assert!(
        adapter_source.contains("TranscriptionContext"),
        "OpenAI adapter must accept portable transcription context"
    );
    assert!(
        adapter_source.contains(".text(\"prompt\""),
        "OpenAI multipart form must include the prior chunk text as prompt"
    );
}
