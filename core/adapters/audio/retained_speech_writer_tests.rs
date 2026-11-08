//! Specifies the background writer that compresses and keeps speech while a meeting is recorded
//! (#88): chunks land in the store in order, `flush` waits for every queued chunk, and dropping
//! the writer still keeps what was queued.

use std::{fs, path::PathBuf, sync::Arc};

use super::{
    opus_speech_tests::voice, retained_audio_store::RetainedAudioStore,
    retained_speech_writer::RetainedSpeechWriter,
};
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        transcript_refinement::ChunkSpan,
    },
    domain::models::MeetingId,
};

/// A store over a fresh folder, and the folder to remove afterwards.
fn store() -> (Arc<RetainedAudioStore>, PathBuf) {
    let path = std::env::temp_dir().join(format!("nt-writer-{}", uuid::Uuid::new_v4()));
    let store = RetainedAudioStore::new(
        path.clone(),
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
    );
    (Arc::new(store), path)
}

fn span(index: u64) -> ChunkSpan {
    ChunkSpan {
        start_ms: index * 1_000,
        end_ms: index * 1_000 + 1_000,
    }
}

#[test]
fn queued_chunks_are_kept_in_order_once_flushed() {
    let (store, path) = store();
    let writer =
        RetainedSpeechWriter::spawn(Arc::clone(&store)).unwrap_or_else(|error| panic!("{error}"));
    let meeting = MeetingId::new("meeting-queued");

    for index in 0..5 {
        writer.append_chunk(&meeting, span(index), None, voice(1.0));
    }
    writer.flush();

    let chunks = store
        .load(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    let spans: Vec<ChunkSpan> = chunks.iter().map(|chunk| chunk.span()).collect();
    assert_eq!(spans, (0..5).map(span).collect::<Vec<_>>());
    assert!(chunks.iter().all(|chunk| chunk.samples().len() == 16_000));
    let _ = fs::remove_dir_all(path);
}

#[test]
fn flushing_with_nothing_queued_returns() {
    let (store, path) = store();
    let writer = RetainedSpeechWriter::spawn(store).unwrap_or_else(|error| panic!("{error}"));

    writer.flush();
    writer.flush();

    let _ = fs::remove_dir_all(path);
}

#[test]
fn dropping_the_writer_still_keeps_what_was_queued() {
    let (store, path) = store();
    let meeting = MeetingId::new("meeting-dropped");
    let writer =
        RetainedSpeechWriter::spawn(Arc::clone(&store)).unwrap_or_else(|error| panic!("{error}"));

    writer.append_chunk(&meeting, span(0), None, voice(2.0));
    drop(writer);

    let chunks = store
        .load(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(chunks.len(), 1);
    let _ = fs::remove_dir_all(path);
}
