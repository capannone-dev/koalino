//! Implements local whisper.cpp transcription behind the portable `TranscriptionEngine` port.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, Transcript, TranscriptionContext},
    },
    ports::ai::TranscriptionEngine,
};

use super::whisper_word_timing::{WhisperTokenTiming, words_from_tokens, words_match_text};

/// Resolves adapter-owned, normalized PCM samples for one portable recording identifier.
///
/// Implementations may read a temporary local audio store, but they must return exactly 16 kHz,
/// mono, floating-point PCM. This is deliberately an adapter-local seam: neither the application
/// nor the transcription port gains a whisper.cpp, file-system, or raw-audio dependency.
pub trait RecordingPcmSource: Send + Sync {
    /// Loads normalized local PCM for transcription.
    ///
    /// Inputs: `audio_recording` contains opaque, portable recording metadata.
    /// Outputs: 16 kHz mono floating-point PCM samples or a portable transcription error.
    /// Side effects: An implementation may read and consume temporary local audio; it must never
    /// log or persist the returned raw samples.
    fn load_pcm_16khz_mono(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Vec<f32>, TranscriptionError>;
}

/// Configures one local whisper.cpp model without exposing binding types to application callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalWhisperCppConfig {
    model_path: PathBuf,
    thread_count: i32,
    use_gpu: bool,
    language: Option<String>,
}

impl LocalWhisperCppConfig {
    /// Validates local inference configuration before a native model context is initialized.
    ///
    /// Inputs: `model_path` names a local GGML/GGUF model; `thread_count` is at least one;
    /// `use_gpu` requests a compiled-in GPU backend; `language` is an optional Whisper language.
    /// Outputs: A validated configuration or a portable error that contains no model contents.
    /// Side effects: Owns path and language metadata only; it does not read or download a model.
    pub fn new(
        model_path: PathBuf,
        thread_count: usize,
        use_gpu: bool,
        language: Option<String>,
    ) -> Result<Self, TranscriptionError> {
        let thread_count =
            i32::try_from(thread_count).map_err(|_| TranscriptionError::InvalidRecording {
                reason: "local transcription thread count exceeds whisper.cpp limits".to_owned(),
            })?;
        if thread_count < 1 {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local transcription requires at least one thread".to_owned(),
            });
        }
        if language
            .as_deref()
            .is_some_and(|configured_language| configured_language.contains('\0'))
        {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local transcription language must not contain a null byte".to_owned(),
            });
        }

        Ok(Self {
            model_path,
            thread_count,
            use_gpu,
            language,
        })
    }
}

/// Implements local transcription with an adapter-owned whisper.cpp model context.
///
/// A mutex serializes native state creation and inference for this one model context. Callers that
/// require non-blocking UI or runtime behavior must invoke this synchronous port on a background
/// worker, as required by the application architecture.
pub struct LocalWhisperCppTranscriptionEngine {
    model_context: Mutex<WhisperContext>,
    thread_count: i32,
    language: Option<String>,
    recording_pcm_source: Arc<dyn RecordingPcmSource>,
}

impl LocalWhisperCppTranscriptionEngine {
    /// Initializes a local whisper.cpp model and retains the adapter-local PCM source.
    ///
    /// Inputs: `configuration` is validated local model metadata; `recording_pcm_source` resolves
    /// normalized local audio for the portable recording ID.
    /// Outputs: A transcription engine or `Unavailable` when the local native model cannot load.
    /// Side effects: Opens and initializes the configured local model; it performs no inference,
    /// network request, model download, or raw-audio logging.
    pub fn new(
        configuration: LocalWhisperCppConfig,
        recording_pcm_source: Arc<dyn RecordingPcmSource>,
    ) -> Result<Self, TranscriptionError> {
        let model_path = configuration
            .model_path
            .to_str()
            .ok_or(TranscriptionError::Unavailable)?;
        let context_parameters = WhisperContextParameters {
            use_gpu: configuration.use_gpu,
            ..WhisperContextParameters::default()
        };
        let model_context = WhisperContext::new_with_params(model_path, context_parameters)
            .map_err(|_| TranscriptionError::Unavailable)?;

        Ok(Self {
            model_context: Mutex::new(model_context),
            thread_count: configuration.thread_count,
            language: configuration.language,
            recording_pcm_source,
        })
    }

    /// Validates that adapter-provided PCM is safe for the native whisper.cpp API.
    ///
    /// Inputs: `pcm_samples` must be normalized 16 kHz mono floating-point samples.
    /// Outputs: Success or a portable invalid-recording error.
    /// Side effects: None; it does not retain or log the raw samples.
    fn validate_pcm(pcm_samples: &[f32]) -> Result<(), TranscriptionError> {
        if pcm_samples.is_empty() {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local audio source returned no PCM samples".to_owned(),
            });
        }
        if pcm_samples.iter().any(|sample| !sample.is_finite()) {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local audio source returned non-finite PCM samples".to_owned(),
            });
        }
        Ok(())
    }
}

impl TranscriptionEngine for LocalWhisperCppTranscriptionEngine {
    /// Transcribes one recording through adapter-owned whisper.cpp inference.
    ///
    /// Inputs: `audio_recording` provides meeting metadata only; `transcription_context` provides
    /// optional prior text; the injected local source resolves 16 kHz mono PCM without exposing
    /// raw audio through the port.
    /// Outputs: A meeting-associated portable transcript, or an unavailable/invalid-recording
    /// error. Native whisper.cpp errors never cross the hexagonal boundary.
    /// Side effects: Reads temporary local audio through the source and runs CPU/GPU inference;
    /// it serializes this engine's model use and neither logs nor persists raw audio.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        let pcm_samples = self
            .recording_pcm_source
            .load_pcm_16khz_mono(audio_recording)?;
        Self::validate_pcm(&pcm_samples)?;

        let model_context = self
            .model_context
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?;
        let mut inference_state = model_context
            .create_state()
            .map_err(|_| TranscriptionError::Unavailable)?;
        let mut inference_parameters = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        inference_parameters.set_n_threads(self.thread_count);
        inference_parameters.set_print_progress(false);
        inference_parameters.set_print_realtime(false);
        inference_parameters.set_print_timestamps(false);
        inference_parameters.set_token_timestamps(true);
        if let Some(language) = transcription_context.language_or(self.language.as_deref()) {
            inference_parameters.set_language(Some(language));
        }
        if let Some(previous_text) = transcription_context.previous_text() {
            inference_parameters.set_initial_prompt(previous_text);
        }
        inference_state
            .full(inference_parameters, &pcm_samples)
            .map_err(|_| TranscriptionError::Unavailable)?;

        let mut transcript_content = String::new();
        let mut spoken_tokens = Vec::new();
        for transcript_segment in inference_state.as_iter() {
            let segment_text = transcript_segment
                .to_str_lossy()
                .map_err(|_| TranscriptionError::Unavailable)?;
            if is_non_speech_marker(&segment_text) {
                continue;
            }
            transcript_content.push_str(&segment_text);
            for token_index in 0..transcript_segment.n_tokens() {
                if let Some(token) = transcript_segment.get_token(token_index)
                    && let Ok(bytes) = token.to_bytes()
                {
                    let timing = token.token_data();
                    spoken_tokens.push((bytes.to_vec(), timing.t0, timing.t1));
                }
            }
        }

        // Timing only steers who said a word: if it ever disagrees with the text, the text wins
        // and the transcript falls back to splitting by time.
        let timed_words =
            words_from_tokens(
                spoken_tokens
                    .iter()
                    .map(|(bytes, t0, t1)| WhisperTokenTiming {
                        bytes,
                        start_units: *t0,
                        end_units: *t1,
                    }),
            );
        let timed_words = if words_match_text(&timed_words, &transcript_content) {
            timed_words
        } else {
            tracing::warn!("whisper word timing does not match its text; ignoring the timing");
            Vec::new()
        };

        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            transcript_content.trim(),
        )
        .with_timed_words(timed_words))
    }
}

/// Recognizes a segment that is only one of whisper.cpp's non-speech markers (Issue #51).
///
/// Inputs: One segment's text.
/// Outputs: `true` for a lone bracketed, parenthesized or starred tag such as `[BLANK_AUDIO]`,
/// `(silence)` or `*applause*`, which would otherwise end up in the transcript; `false` for
/// anything with words outside such a tag, and for empty text.
/// Side effects: None.
pub fn is_non_speech_marker(segment_text: &str) -> bool {
    let trimmed = segment_text.trim();
    let mut characters = trimmed.chars();
    let (Some(opening), Some(closing)) = (characters.next(), characters.next_back()) else {
        return false;
    };
    let inner = characters.as_str();
    let expected_closing = match opening {
        '[' => ']',
        '(' => ')',
        '*' => '*',
        _ => return false,
    };
    closing == expected_closing
        && !inner.contains([opening, expected_closing])
        && !inner.trim().is_empty()
}
