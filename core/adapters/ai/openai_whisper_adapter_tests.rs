//! Defines mock-first contracts for the OpenAI Whisper transcription adapter.

use std::sync::{Arc, Mutex};

use crate::{
    adapters::ai::openai_whisper_adapter::{
        AudioFileSource, OPENAI_MAX_AUDIO_BYTES, OpenAiHttpClient, OpenAiHttpResponse,
        OpenAiTranscriptionOptions, OpenAiWhisperConfig, OpenAiWhisperTranscriptionEngine,
    },
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, DeviceId, MeetingId, RecordingId, TranscriptionContext},
        transcription_language::TranscriptionLanguage,
    },
    ports::ai::TranscriptionEngine,
};

type CapturedRequest = (String, String, String, Vec<u8>, Option<String>);

#[derive(Clone)]
struct MockHttpClient {
    responses: Arc<Mutex<Vec<OpenAiHttpResponse>>>,
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    languages: Arc<Mutex<Vec<Option<String>>>>,
}
impl MockHttpClient {
    fn new(responses: Vec<OpenAiHttpResponse>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(responses)),
            requests: Arc::new(Mutex::new(Vec::new())),
            languages: Arc::new(Mutex::new(Vec::new())),
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
        options: OpenAiTranscriptionOptions<'_>,
    ) -> Result<OpenAiHttpResponse, TranscriptionError> {
        self.requests
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?
            .push((
                authorization.to_owned(),
                filename.to_owned(),
                content_type.to_owned(),
                audio_bytes,
                options.prompt().map(str::to_owned),
            ));
        self.languages
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?
            .push(options.language().map(str::to_owned));
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
        .transcribe(&recording(), &TranscriptionContext::default())
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
        engine.transcribe(&recording(), &TranscriptionContext::default()),
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
            .transcribe(&recording(), &TranscriptionContext::default())
            .err()
            .unwrap_or_else(|| panic!("HTTP failure should be portable"));
        assert!(!error.to_string().contains("test-secret"));
    }
}

/// Verifies cloud transcription forwards prior chunk text as OpenAI's multipart prompt field.
///
/// Inputs: A portable context with the previous meeting chunk transcript and an injected HTTP
/// client that records safe request metadata.
/// Outputs: Confirms the recorded multipart request carries the exact prior text as `prompt`.
/// Side effects: Invokes the injected mock only; it makes no HTTP request or exposes secrets.
#[test]
fn openai_adapter_receives_context_previous_text_and_includes_multipart_prompt_field() {
    let http = MockHttpClient::new(vec![OpenAiHttpResponse::success(r#"{"text":"next"}"#)]);
    let requests = http.requests.clone();
    let engine = OpenAiWhisperTranscriptionEngine::with_dependencies(
        OpenAiWhisperConfig::new("test-secret", "whisper-1", None)
            .unwrap_or_else(|error| panic!("config should validate: {error}")),
        Arc::new(StaticAudioSource(vec![1, 2, 3])),
        Arc::new(http),
    );
    let transcription_context = TranscriptionContext::new(Some("previous chunk text".to_owned()));

    engine
        .transcribe(&recording(), &transcription_context)
        .unwrap_or_else(|error| panic!("mock success should transcribe: {error}"));

    let recorded_requests = requests
        .lock()
        .unwrap_or_else(|_| panic!("mock requests should be readable"));
    assert_eq!(
        recorded_requests[0].4.as_deref(),
        Some("previous chunk text")
    );
}

/// A chunk's language replaces the configured one, and AUTO sends none, so the API detects it.
#[test]
fn a_chunks_language_replaces_the_configured_one() {
    let http = MockHttpClient::new(vec![
        OpenAiHttpResponse::success(r#"{"text":"c"}"#),
        OpenAiHttpResponse::success(r#"{"text":"b"}"#),
        OpenAiHttpResponse::success(r#"{"text":"a"}"#),
    ]);
    let languages = http.languages.clone();
    let engine = OpenAiWhisperTranscriptionEngine::with_dependencies(
        OpenAiWhisperConfig::new("test-secret", "whisper-1", Some("it".to_owned()))
            .unwrap_or_else(|error| panic!("config should validate: {error}")),
        Arc::new(StaticAudioSource(vec![1, 2, 3])),
        Arc::new(http),
    );

    for language in [
        None,
        TranscriptionLanguage::parse("en").ok(),
        Some(TranscriptionLanguage::Auto),
    ] {
        engine
            .transcribe(
                &recording(),
                &TranscriptionContext::new(None).with_language(language),
            )
            .unwrap_or_else(|error| panic!("mock success should transcribe: {error}"));
    }

    assert_eq!(
        *languages
            .lock()
            .unwrap_or_else(|_| panic!("mock languages should be readable")),
        vec![Some("it".to_owned()), Some("en".to_owned()), None]
    );
}
