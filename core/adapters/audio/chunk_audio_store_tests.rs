//! Specifies the temporary chunk-audio store: WAV/PCM encoding, delete-on-read, and clearing.

use crate::{
    adapters::audio::chunk_audio_store::{ChunkAudioStore, encode_wav_pcm16},
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, DeviceId, MeetingId, RecordingId},
    },
};

/// Builds recording metadata for one chunk ID.
fn recording(recording_id: &str) -> AudioRecording {
    AudioRecording::new(
        RecordingId::new(recording_id),
        MeetingId::new("meeting-audio"),
        DeviceId::new("device-test"),
    )
}

/// Verifies the WAV encoder writes a canonical 44-byte header for 16-bit mono PCM.
#[test]
fn wav_encoding_has_a_canonical_pcm16_header() {
    let wav_bytes = encode_wav_pcm16(&[0, 1, -1, i16::MAX], 16_000);

    assert_eq!(wav_bytes.len(), 44 + 8);
    assert_eq!(&wav_bytes[0..4], b"RIFF");
    assert_eq!(&wav_bytes[4..8], &(36_u32 + 8).to_le_bytes());
    assert_eq!(&wav_bytes[8..16], b"WAVEfmt ");
    assert_eq!(&wav_bytes[20..22], &1_u16.to_le_bytes());
    assert_eq!(&wav_bytes[22..24], &1_u16.to_le_bytes());
    assert_eq!(&wav_bytes[24..28], &16_000_u32.to_le_bytes());
    assert_eq!(&wav_bytes[28..32], &32_000_u32.to_le_bytes());
    assert_eq!(&wav_bytes[34..36], &16_u16.to_le_bytes());
    assert_eq!(&wav_bytes[36..40], b"data");
    assert_eq!(&wav_bytes[40..44], &8_u32.to_le_bytes());
    assert_eq!(&wav_bytes[44..46], &0_i16.to_le_bytes());
    assert_eq!(&wav_bytes[50..52], &i16::MAX.to_le_bytes());
}

/// Verifies loading a chunk returns its WAV and deletes the raw audio (PII) from the store.
#[test]
fn loading_a_chunk_deletes_its_audio() {
    let store = ChunkAudioStore::new();
    store.insert(&RecordingId::new("chunk-1"), vec![5; 320]);
    assert_eq!(store.len(), 1);

    let wav_bytes = store
        .load_wav(&recording("chunk-1"))
        .unwrap_or_else(|error| panic!("stored chunk should load: {error}"));
    assert_eq!(wav_bytes.len(), 44 + 640);
    assert!(store.is_empty());
    assert!(matches!(
        store.load_wav(&recording("chunk-1")),
        Err(TranscriptionError::InvalidRecording { .. })
    ));
}

/// Verifies an unknown chunk ID is rejected instead of returning someone else's audio.
#[test]
fn loading_an_unknown_chunk_is_rejected() {
    let store = ChunkAudioStore::new();
    store.insert(&RecordingId::new("chunk-1"), vec![5; 320]);

    assert!(matches!(
        store.load_wav(&recording("chunk-2")),
        Err(TranscriptionError::InvalidRecording { .. })
    ));
    assert_eq!(store.len(), 1);
}

/// Verifies clearing removes audio left by chunks that were never transcribed.
#[test]
fn clearing_discards_unprocessed_audio() {
    let store = ChunkAudioStore::new();
    store.insert(&RecordingId::new("chunk-1"), vec![1; 10]);
    store.insert(&RecordingId::new("chunk-2"), vec![2; 10]);

    assert_eq!(store.clear(), 2);
    assert!(store.is_empty());
}

/// Verifies the local-model PCM source normalizes samples to [-1, 1) and deletes on read.
#[cfg(feature = "local-ai")]
#[test]
fn loading_pcm_normalizes_samples_and_deletes_audio() {
    use crate::adapters::ai::local_whisper_cpp::RecordingPcmSource;

    let store = ChunkAudioStore::new();
    store.insert(&RecordingId::new("chunk-1"), vec![i16::MIN, 0, 16_384]);

    let pcm = store
        .load_pcm_16khz_mono(&recording("chunk-1"))
        .unwrap_or_else(|error| panic!("stored chunk should load: {error}"));
    assert_eq!(pcm, vec![-1.0, 0.0, 0.5]);
    assert!(store.is_empty());
}

/// Verifies discarding by ID or prefix removes only the matching chunks.
#[test]
fn discarding_removes_only_matching_chunks() {
    let store = ChunkAudioStore::new();
    store.insert(&RecordingId::new("local-a-chunk-0"), vec![1]);
    store.insert(&RecordingId::new("local-a-chunk-1"), vec![2]);
    store.insert(&RecordingId::new("teams-chunk-0"), vec![3]);

    assert!(store.discard(&RecordingId::new("teams-chunk-0")));
    assert!(!store.discard(&RecordingId::new("teams-chunk-0")));
    store.insert(&RecordingId::new("teams-chunk-1"), vec![4]);
    assert_eq!(store.discard_with_prefix("local-a-"), 2);
    assert_eq!(store.len(), 1);
    assert!(store.load_wav(&recording("teams-chunk-1")).is_ok());
}
