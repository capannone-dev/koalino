//! Holds each live recording chunk's raw PCM in memory until a transcription engine reads it.
//!
//! The transcription port receives only `AudioRecording` metadata; engines resolve the audio
//! through adapter-side sources. `ChunkAudioStore` implements both of them: `AudioFileSource`
//! (WAV, for the OpenAI adapter) and, with `local-ai`, `RecordingPcmSource` (f32 PCM, for
//! whisper.cpp). Entries are keyed by the chunk's `RecordingId`, removed on the first read, and
//! wiped from memory, so raw audio (PII) never outlives its transcription. Recording controllers
//! call `discard_with_prefix` on stop to discard audio of their chunks that were never
//! transcribed, leaving other producers (the Teams bridge) untouched. Nothing here
//! writes to disk or logs samples.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use zeroize::Zeroize;

use crate::{
    adapters::ai::openai_whisper_adapter::AudioFileSource,
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, RecordingId},
    },
};

/// Sample rate of every stored chunk.
const CHUNK_SAMPLE_RATE_HZ: u32 = 16_000;
/// Size of the canonical RIFF/WAVE header written by `encode_wav_pcm16`.
const WAV_HEADER_LENGTH: usize = 44;

/// In-memory, delete-on-read store of 16 kHz mono PCM per chunk.
#[derive(Default)]
pub struct ChunkAudioStore {
    /// Raw samples keyed by the chunk's recording ID.
    chunk_samples: Mutex<HashMap<String, Vec<i16>>>,
}

impl ChunkAudioStore {
    /// Creates an empty store.
    ///
    /// Inputs: None.
    /// Outputs: The store.
    /// Side effects: None.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores one chunk's samples until an engine reads them.
    ///
    /// Inputs: The chunk's unique `recording_id` and its 16 kHz mono samples.
    /// Outputs: None. A previous entry with the same ID is replaced and wiped.
    /// Side effects: Retains raw audio in memory.
    pub fn insert(&self, recording_id: &RecordingId, samples: Vec<i16>) {
        if let Some(mut replaced) = self
            .lock_entries()
            .insert(recording_id.as_str().to_owned(), samples)
        {
            replaced.zeroize();
        }
    }

    /// Returns how many chunks are waiting to be read.
    pub fn len(&self) -> usize {
        self.lock_entries().len()
    }

    /// Reports whether no chunk audio is held.
    pub fn is_empty(&self) -> bool {
        self.lock_entries().is_empty()
    }

    /// Discards every unread chunk.
    ///
    /// Inputs: None.
    /// Outputs: How many chunks were discarded.
    /// Side effects: Wipes and frees all retained raw audio.
    pub fn clear(&self) -> usize {
        let mut entries = self.lock_entries();
        let discarded_count = entries.len();
        for samples in entries.values_mut() {
            samples.zeroize();
        }
        entries.clear();
        discarded_count
    }

    /// Discards one chunk's audio, e.g. after its transcription failed before reading it.
    ///
    /// Inputs: The chunk's recording ID.
    /// Outputs: Whether audio was still held for it.
    /// Side effects: Wipes and frees that chunk's raw audio.
    pub fn discard(&self, recording_id: &RecordingId) -> bool {
        match self.lock_entries().remove(recording_id.as_str()) {
            Some(mut samples) => {
                samples.zeroize();
                true
            }
            None => false,
        }
    }

    /// Discards every unread chunk whose recording ID starts with `recording_id_prefix`.
    ///
    /// Inputs: The prefix shared by one producer's chunk IDs (e.g. one local recording), so
    /// other producers sharing the store (the Teams bridge) keep their audio.
    /// Outputs: How many chunks were discarded.
    /// Side effects: Wipes and frees the matching raw audio.
    pub fn discard_with_prefix(&self, recording_id_prefix: &str) -> usize {
        let mut entries = self.lock_entries();
        let matching_ids: Vec<String> = entries
            .keys()
            .filter(|recording_id| recording_id.starts_with(recording_id_prefix))
            .cloned()
            .collect();
        for recording_id in &matching_ids {
            if let Some(mut samples) = entries.remove(recording_id) {
                samples.zeroize();
            }
        }
        matching_ids.len()
    }

    /// Removes and returns one chunk's samples.
    ///
    /// Inputs: The chunk's recording metadata.
    /// Outputs: The samples, or `InvalidRecording` when the chunk is unknown or already read.
    /// Side effects: Deletes the entry.
    fn take_samples(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Vec<i16>, TranscriptionError> {
        self.lock_entries()
            .remove(audio_recording.recording_id().as_str())
            .ok_or_else(|| TranscriptionError::InvalidRecording {
                reason: "chunk audio is not available".to_owned(),
            })
    }

    /// Locks the entry map, recovering it if a previous holder panicked.
    fn lock_entries(&self) -> MutexGuard<'_, HashMap<String, Vec<i16>>> {
        self.chunk_samples
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl AudioFileSource for ChunkAudioStore {
    /// Returns the chunk as a 16-bit PCM WAV file and deletes it from the store.
    ///
    /// Inputs: The chunk's recording metadata.
    /// Outputs: WAV bytes, or `InvalidRecording` when the chunk is unknown or already read.
    /// Side effects: Deletes and wipes the stored samples.
    fn load_wav(&self, audio_recording: &AudioRecording) -> Result<Vec<u8>, TranscriptionError> {
        let mut samples = self.take_samples(audio_recording)?;
        let wav_bytes = encode_wav_pcm16(&samples, CHUNK_SAMPLE_RATE_HZ);
        samples.zeroize();
        Ok(wav_bytes)
    }
}

#[cfg(feature = "local-ai")]
impl crate::adapters::ai::local_whisper_cpp::RecordingPcmSource for ChunkAudioStore {
    /// Returns the chunk as normalized f32 PCM and deletes it from the store.
    ///
    /// Inputs: The chunk's recording metadata.
    /// Outputs: Samples in [-1, 1), or `InvalidRecording` when the chunk is unknown or already read.
    /// Side effects: Deletes and wipes the stored samples.
    fn load_pcm_16khz_mono(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Vec<f32>, TranscriptionError> {
        let mut samples = self.take_samples(audio_recording)?;
        let normalized = samples
            .iter()
            .map(|sample| f32::from(*sample) / 32_768.0)
            .collect();
        samples.zeroize();
        Ok(normalized)
    }
}

/// Encodes mono 16-bit PCM as a canonical RIFF/WAVE file.
///
/// Inputs: `samples` (mono) and their `sample_rate_hz`.
/// Outputs: A 44-byte header followed by little-endian samples. Inputs above 4 GiB are not
/// expected for chunks; the size fields saturate instead of wrapping.
/// Side effects: None; the returned bytes are raw audio and must never be logged.
pub fn encode_wav_pcm16(samples: &[i16], sample_rate_hz: u32) -> Vec<u8> {
    let data_length = u32::try_from(samples.len().saturating_mul(2)).unwrap_or(u32::MAX);
    let mut wav_bytes = Vec::with_capacity(WAV_HEADER_LENGTH + samples.len() * 2);
    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&data_length.saturating_add(36).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16_u32.to_le_bytes()); // fmt chunk length
    wav_bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM format
    wav_bytes.extend_from_slice(&1_u16.to_le_bytes()); // channels
    wav_bytes.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate_hz.saturating_mul(2).to_le_bytes()); // byte rate
    wav_bytes.extend_from_slice(&2_u16.to_le_bytes()); // block align
    wav_bytes.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&data_length.to_le_bytes());
    for sample in samples {
        wav_bytes.extend_from_slice(&sample.to_le_bytes());
    }
    wav_bytes
}
