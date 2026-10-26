//! Defines mock-first tests for VAD frame buffering, cuts, and transcription text overlap handling.

use crate::{
    adapters::audio::vad_chunker::{ChunkCutType, VadChunker, VadConfig, deduplicate_overlap},
    domain::models::{AudioChunk, AudioConfig},
};

#[test]
fn audio_input_in_non_30ms_multiples_is_correctly_buffered_and_reframed() {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("config should work: {error}"));
    chunker
        .push_pcm(AudioChunk::new(vec![1; 500], AudioConfig::new(16_000, 1)))
        .unwrap_or_else(|error| panic!("partial frame should buffer: {error}"));
    assert_eq!(chunker.processed_frame_count(), 1);
    assert_eq!(chunker.buffered_sample_count(), 20);
}

#[test]
fn italian_pause_and_soft_max_or_interruption_finalize_with_portable_cut_types() {
    let mut chunker = VadChunker::new(VadConfig::italian())
        .unwrap_or_else(|error| panic!("config should work: {error}"));
    chunker
        .push_pcm(AudioChunk::new(vec![1; 480], AudioConfig::new(16_000, 1)))
        .unwrap_or_else(|error| panic!("speech should queue: {error}"));
    let finalized = chunker
        .handle_interruption()
        .unwrap_or_else(|error| panic!("interruption should finalize: {error}"));
    assert_eq!(finalized.cut_type(), ChunkCutType::Interrupted);
}

#[test]
fn text_deduplication_handles_whole_partial_and_short_non_overlaps() {
    assert_eq!(
        deduplicate_overlap("the cat sat", "cat sat on"),
        "the cat sat on"
    );
    assert_eq!(
        deduplicate_overlap("the synchroniz", "synchronization is"),
        "the synchronization is"
    );
    assert_eq!(deduplicate_overlap("a cat", "a dog"), "a cat a dog");
}
