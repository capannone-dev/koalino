//! Implements OpenAI Whisper multipart transcription behind the portable engine port.

use std::sync::Arc;

use serde::Deserialize;

use crate::{
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, Transcript},
    },
    ports::ai::TranscriptionEngine,
};

/// OpenAI's documented maximum audio upload size for one transcription request.
pub const OPENAI_MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;
const OPENAI_TRANSCRIPTIONS_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

/// Resolves adapter-owned raw PCM only after it has been wrapped as a valid WAV file.
pub trait AudioFileSource: Send + Sync {
    /// Loads a RIFF/WAV-compatible byte stream for one portable recording.
    fn load_wav(&self, audio_recording: &AudioRecording) -> Result<Vec<u8>, TranscriptionError>;
}

/// Holds only safe OpenAI endpoint response details for adapter-local parsing.
#[derive(Clone, Debug)]
pub struct OpenAiHttpResponse {
    status: u16,
    body: String,
}
impl OpenAiHttpResponse {
    /// Creates a successful mock/provider response body.
    pub fn success(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
        }
    }
    /// Creates an error response without exposing its body to portable errors or logs.
    pub fn failure(status: u16) -> Self {
        Self {
            status,
            body: String::new(),
        }
    }
}

/// Performs adapter-private multipart HTTP calls without leaking reqwest into applications.
pub trait OpenAiHttpClient: Send + Sync {
    /// Posts one WAV file with safe metadata to the OpenAI transcription endpoint.
    fn post_transcription(
        &self,
        authorization: &str,
        filename: &str,
        content_type: &str,
        audio_bytes: Vec<u8>,
        model: &str,
        language: Option<&str>,
    ) -> Result<OpenAiHttpResponse, TranscriptionError>;
}

/// Uses reqwest's blocking client; callers must execute the sync port on a background worker.
pub struct ReqwestOpenAiHttpClient {
    client: reqwest::blocking::Client,
}
impl ReqwestOpenAiHttpClient {
    /// Creates an HTTP client with reqwest defaults and no API key retained in logs.
    pub fn new() -> Result<Self, TranscriptionError> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .build()
                .map_err(|_| TranscriptionError::Unavailable)?,
        })
    }
}
impl OpenAiHttpClient for ReqwestOpenAiHttpClient {
    fn post_transcription(
        &self,
        authorization: &str,
        filename: &str,
        content_type: &str,
        audio_bytes: Vec<u8>,
        model: &str,
        language: Option<&str>,
    ) -> Result<OpenAiHttpResponse, TranscriptionError> {
        let part = reqwest::blocking::multipart::Part::bytes(audio_bytes)
            .file_name(filename.to_owned())
            .mime_str(content_type)
            .map_err(|_| TranscriptionError::InvalidRecording {
                reason: "invalid audio MIME type".to_owned(),
            })?;
        let mut form = reqwest::blocking::multipart::Form::new()
            .part("file", part)
            .text("model", model.to_owned())
            .text("response_format", "verbose_json");
        if let Some(language) = language {
            form = form.text("language", language.to_owned());
        }
        let response = self
            .client
            .post(OPENAI_TRANSCRIPTIONS_URL)
            .header(reqwest::header::AUTHORIZATION, authorization)
            .multipart(form)
            .send()
            .map_err(|_| TranscriptionError::Unavailable)?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|_| TranscriptionError::Unavailable)?;
        Ok(OpenAiHttpResponse { status, body })
    }
}

/// Validated adapter-private OpenAI Whisper selection and secret.
#[derive(Clone, Debug)]
pub struct OpenAiWhisperConfig {
    api_key: String,
    model: String,
    language: Option<String>,
}
impl OpenAiWhisperConfig {
    /// Validates configuration supplied by environment loading or a secure keychain bridge.
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        language: Option<String>,
    ) -> Result<Self, TranscriptionError> {
        let api_key = api_key.into();
        let model = model.into();
        if api_key.is_empty() || model.is_empty() || api_key.contains('\n') || model.contains('\0')
        {
            return Err(TranscriptionError::InvalidRecording {
                reason: "invalid cloud transcription configuration".to_owned(),
            });
        }
        Ok(Self {
            api_key,
            model,
            language,
        })
    }
    /// Loads OpenAI configuration from standard environment variables without logging values.
    pub fn from_environment() -> Result<Self, TranscriptionError> {
        let key = std::env::var("OPENAI_API_KEY").map_err(|_| TranscriptionError::Unavailable)?;
        let model = std::env::var("NOTE_TAKER_OPENAI_WHISPER_MODEL")
            .unwrap_or_else(|_| "whisper-1".to_owned());
        Self::new(key, model, std::env::var("NOTE_TAKER_OPENAI_LANGUAGE").ok())
    }
}

/// Implements portable transcription while keeping provider HTTP details adapter-local.
pub struct OpenAiWhisperTranscriptionEngine {
    configuration: OpenAiWhisperConfig,
    audio_file_source: Arc<dyn AudioFileSource>,
    http_client: Arc<dyn OpenAiHttpClient>,
}
impl OpenAiWhisperTranscriptionEngine {
    /// Creates an engine from environment configuration and an adapter-owned WAV source.
    pub fn from_environment(
        audio_file_source: Arc<dyn AudioFileSource>,
    ) -> Result<Self, TranscriptionError> {
        Ok(Self {
            configuration: OpenAiWhisperConfig::from_environment()?,
            audio_file_source,
            http_client: Arc::new(ReqwestOpenAiHttpClient::new()?),
        })
    }
    /// Injects adapter-private dependencies for deterministic no-network tests.
    pub fn with_dependencies(
        configuration: OpenAiWhisperConfig,
        audio_file_source: Arc<dyn AudioFileSource>,
        http_client: Arc<dyn OpenAiHttpClient>,
    ) -> Self {
        Self {
            configuration,
            audio_file_source,
            http_client,
        }
    }
}
#[derive(Deserialize)]
struct WhisperResponse {
    text: String,
}
impl TranscriptionEngine for OpenAiWhisperTranscriptionEngine {
    /// Uploads one adapter-owned WAV file and maps the response to the portable transcript contract.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        let audio_bytes = self.audio_file_source.load_wav(audio_recording)?;
        if audio_bytes.len() > OPENAI_MAX_AUDIO_BYTES {
            return Err(TranscriptionError::PayloadTooLarge {
                maximum_bytes: OPENAI_MAX_AUDIO_BYTES,
            });
        }
        let authorization = format!("Bearer {}", self.configuration.api_key);
        let response = self.http_client.post_transcription(
            &authorization,
            "audio_chunk.wav",
            "audio/wav",
            audio_bytes,
            &self.configuration.model,
            self.configuration.language.as_deref(),
        )?;
        match response.status {
            200..=299 => {
                let parsed: WhisperResponse = serde_json::from_str(&response.body)
                    .map_err(|_| TranscriptionError::Unavailable)?;
                Ok(Transcript::new(
                    audio_recording.meeting_id().clone(),
                    parsed.text.trim(),
                ))
            }
            429 => Err(TranscriptionError::RateLimited),
            _ => Err(TranscriptionError::Unavailable),
        }
    }
}
