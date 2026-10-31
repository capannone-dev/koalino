//! Falls back to the local model when cloud transcription fails, so the app still works offline.
//!
//! Audio is never queued for a later upload. The local model loads only when first needed, since
//! someone who chose OpenAI may never need it.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, ExternalMediaChunk, Transcript, TranscriptionContext},
    },
    ports::{ai::TranscriptionEngine, remote_transcription::ChunkSamples},
};

/// Whether the last chunk fell back to the local model, for Settings and the recording screen.
static CLOUD_FALLBACK_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Reports whether the last chunk had to use the local model. Side effects: None.
pub fn cloud_fallback_active() -> bool {
    CLOUD_FALLBACK_ACTIVE.load(Ordering::Relaxed)
}

/// Clears the indicator when a new recording starts. Side effects: Updates the indicator.
pub fn reset_cloud_fallback() {
    CLOUD_FALLBACK_ACTIVE.store(false, Ordering::Relaxed);
}

/// Loads the local engine when first needed.
pub type LocalEngineLoader<Local> =
    Box<dyn Fn() -> Result<Local, TranscriptionError> + Send + Sync>;

/// Tries the cloud engine, then the local one.
pub struct CloudFallbackEngine<Cloud, Local> {
    cloud: Cloud,
    load_local: LocalEngineLoader<Local>,
    local: Mutex<Option<Local>>,
    audio_store: Arc<ChunkAudioStore>,
}

impl<Cloud, Local> CloudFallbackEngine<Cloud, Local> {
    /// Creates the engine.
    ///
    /// Inputs: The cloud engine, a loader for the local one, and the store both read chunks from.
    /// Outputs: The engine. Side effects: None; nothing loads yet.
    pub fn new(
        cloud: Cloud,
        load_local: LocalEngineLoader<Local>,
        audio_store: Arc<ChunkAudioStore>,
    ) -> Self {
        Self {
            cloud,
            load_local,
            local: Mutex::new(None),
            audio_store,
        }
    }
}

impl<Cloud: TranscriptionEngine, Local: TranscriptionEngine> TranscriptionEngine
    for CloudFallbackEngine<Cloud, Local>
{
    /// Transcribes with the cloud engine, and locally if it is unreachable or refuses.
    ///
    /// Inputs: The chunk's metadata and context.
    /// Outputs: A transcript; the cloud error when the local model can't run either. An invalid
    /// recording is returned as is, since the local engine would fail too.
    /// Side effects: Sends audio to the provider; on failure restores the chunk and may load the
    /// local model. Updates the fallback indicator.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        // The cloud engine consumes the chunk, and the local one needs it if the upload fails.
        let kept_samples = self
            .audio_store
            .copy_samples(audio_recording.recording_id());
        let cloud_error = match self
            .cloud
            .transcribe(audio_recording, transcription_context)
        {
            Ok(transcript) => {
                CLOUD_FALLBACK_ACTIVE.store(false, Ordering::Relaxed);
                return Ok(transcript);
            }
            Err(error @ (TranscriptionError::Unavailable | TranscriptionError::RateLimited)) => {
                error
            }
            Err(error) => return Err(error),
        };
        let Some(samples) = kept_samples else {
            return Err(cloud_error);
        };
        self.audio_store
            .insert(audio_recording.recording_id(), samples);
        let mut local = self
            .local
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?;
        if local.is_none() {
            match (self.load_local)() {
                Ok(engine) => *local = Some(engine),
                Err(_) => {
                    self.audio_store.discard(audio_recording.recording_id());
                    return Err(cloud_error);
                }
            }
        }
        let Some(engine) = local.as_ref() else {
            return Err(cloud_error);
        };
        tracing::warn!("Cloud transcription failed; this chunk uses the local model");
        CLOUD_FALLBACK_ACTIVE.store(true, Ordering::Relaxed);
        engine.transcribe(audio_recording, transcription_context)
    }

    /// Sends external media to the cloud engine.
    fn transcribe_external_media(
        &self,
        media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        self.cloud
            .transcribe_external_media(media_chunk, audio_recording)
    }
}
