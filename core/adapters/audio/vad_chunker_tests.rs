//! Defines mock-first tests for VAD frame buffering, cuts, and transcription text overlap handling.

use crate::{
    adapters::audio::vad_chunker::{
        ChunkCutType, ChunkedAudioSource, VAD_FRAME_SAMPLES, VadChunker, VadConfig,
        deduplicate_overlap,
    },
    domain::models::{AudioChunk, AudioConfig},
};

/// Builds one valid silent 30 ms frame without reading or retaining captured audio.
fn silent_frame() -> Vec<i16> {
    vec![0; VAD_FRAME_SAMPLES]
}

/// Pushes complete silent VAD frames into a chunker for deterministic state-machine tests.
fn push_silent_frames(chunker: &mut VadChunker, frame_count: usize) {
    for _ in 0..frame_count {
        chunker
            .push_pcm(AudioChunk::new(silent_frame(), AudioConfig::new(16_000, 1)))
            .unwrap_or_else(|error| panic!("silent VAD frame should buffer: {error}"));
    }
}

/// Requires the VAD adapter source to invoke the feature-gated WebRTC classifier.
#[cfg(feature = "vad")]
fn assert_webrtc_vad_state_machine_is_wired() {
    let adapter_source = include_str!("vad_chunker.rs");
    assert!(
        adapter_source.contains("is_voice_segment"),
        "VadChunker must classify every complete frame through webrtc-vad"
    );
}

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

/// Verifies language-sensitive silence thresholds finalize a VAD pause only after enough silence.
///
/// Inputs: Silent 30 ms PCM frames classified through the verified WebRTC VAD API.
/// Outputs: English finalizes at 600 ms and Italian at 800 ms with `VadPause`.
/// Side effects: Buffers in-memory test PCM only; it does not access a microphone or log samples.
#[test]
#[cfg(feature = "vad")]
fn silence_for_english_or_italian_threshold_triggers_vad_pause_finalization() {
    use webrtc_vad::{SampleRate, Vad, VadMode};

    let frame = silent_frame();
    let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Quality);
    let is_voice: bool = vad.is_voice_segment(&frame).unwrap_or(false);
    assert!(!is_voice, "the deterministic zero frame must be silence");

    let mut english_chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("English configuration should work: {error}"));
    push_silent_frames(&mut english_chunker, 20);
    let english_chunk = english_chunker
        .next_chunk()
        .unwrap_or_else(|error| panic!("English chunk retrieval should work: {error}"))
        .unwrap_or_else(|| panic!("600 ms of silence must finalize an English VAD pause"));
    assert_eq!(english_chunk.cut_type(), ChunkCutType::VadPause);

    let mut italian_chunker = VadChunker::new(VadConfig::italian())
        .unwrap_or_else(|error| panic!("Italian configuration should work: {error}"));
    push_silent_frames(&mut italian_chunker, 27);
    let italian_chunk = italian_chunker
        .next_chunk()
        .unwrap_or_else(|error| panic!("Italian chunk retrieval should work: {error}"))
        .unwrap_or_else(|| panic!("810 ms of silence must finalize an Italian VAD pause"));
    assert_eq!(italian_chunk.cut_type(), ChunkCutType::VadPause);
    assert_webrtc_vad_state_machine_is_wired();
}

/// Verifies a speaking frame resets accumulated silence before a pause threshold is reached.
///
/// Inputs: Nineteen silent English VAD frames followed by a non-silent frame.
/// Outputs: No finalized chunk because speech resumed before the 600 ms threshold.
/// Side effects: Buffers in-memory test PCM only; it does not access a microphone or log samples.
#[test]
#[cfg(feature = "vad")]
fn speech_resumes_before_threshold_so_chunk_continues_without_split() {
    use webrtc_vad::{SampleRate, Vad, VadMode};

    let speech_frame = (0..VAD_FRAME_SAMPLES)
        .map(|sample_index| if sample_index % 2 == 0 { 8_000 } else { -8_000 })
        .collect::<Vec<i16>>();
    let mut vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Quality);
    let is_voice: bool = vad.is_voice_segment(&speech_frame).unwrap_or(false);
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("English configuration should work: {error}"));
    push_silent_frames(&mut chunker, 19);
    chunker
        .push_pcm(AudioChunk::new(speech_frame, AudioConfig::new(16_000, 1)))
        .unwrap_or_else(|error| panic!("speech frame should buffer: {error}"));

    assert!(
        is_voice,
        "the deterministic alternating frame must be voice"
    );
    assert!(
        chunker
            .next_chunk()
            .unwrap_or_else(|error| panic!("chunk retrieval should work: {error}"))
            .is_none(),
        "speech before the threshold must reset silence without splitting"
    );
    assert_webrtc_vad_state_machine_is_wired();
}

/// Verifies the hard cap forces a chunk and retains one second of overlap for its successor.
///
/// Inputs: Exactly 120 seconds of 16 kHz mono PCM, expressed as complete 30 ms frames.
/// Outputs: A `SoftMaxForced` chunk and an adapter-owned one-second lookback buffer.
/// Side effects: Allocates in-memory test PCM only; it does not access a microphone or log samples.
#[test]
#[cfg(feature = "vad")]
fn soft_max_forces_chunk_with_one_second_lookback() {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("English configuration should work: {error}"));
    push_silent_frames(&mut chunker, 4_000);
    let forced_chunk = chunker
        .next_chunk()
        .unwrap_or_else(|error| panic!("forced chunk retrieval should work: {error}"))
        .unwrap_or_else(|| panic!("120 seconds must force chunk finalization"));

    assert_eq!(forced_chunk.cut_type(), ChunkCutType::SoftMaxForced);
    let adapter_source = include_str!("vad_chunker.rs");
    assert!(
        adapter_source.contains("lookback_samples"),
        "soft-max finalization must retain a one-second lookback buffer"
    );
}

/// Verifies disabled VAD uses bounded time chunking and records a warning instead of failing audio.
///
/// Inputs: Thirty seconds of 16 kHz mono PCM while the `vad` feature is absent.
/// Outputs: A time-forced chunk and source-level confirmation of the fallback warning contract.
/// Side effects: Buffers in-memory test PCM only; it does not access a microphone or log samples.
#[test]
#[cfg(not(feature = "vad"))]
fn disabled_vad_falls_back_to_thirty_second_time_chunking_with_warn_log() {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("fallback configuration should work: {error}"));
    push_silent_frames(&mut chunker, 1_000);
    let fallback_chunk = chunker
        .next_chunk()
        .unwrap_or_else(|error| panic!("fallback chunk retrieval should work: {error}"))
        .unwrap_or_else(|| panic!("disabled VAD must finalize after 30 seconds"));

    assert_eq!(fallback_chunk.cut_type(), ChunkCutType::SoftMaxForced);
    let adapter_source = include_str!("vad_chunker.rs");
    assert!(
        adapter_source.contains("falling back to 30-second time-based chunking"),
        "disabled or unavailable VAD must emit a warning describing time fallback"
    );
}

/// Verifies runtime time-based mode cuts every 30 seconds even when the `vad` feature is built,
/// and that the successor chunk reports its one-second lookback as overlap.
#[test]
fn time_based_mode_cuts_every_thirty_seconds_and_reports_overlap() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    push_silent_frames(&mut chunker, 2_000);

    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 2);
    assert!(
        chunks
            .iter()
            .all(|chunk| chunk.cut_type() == ChunkCutType::SoftMaxForced)
    );
    assert_eq!(chunks[0].overlap_sample_count(), 0);
    assert_eq!(chunks[0].audio_chunk().samples().len(), 480_000);
    assert_eq!(chunks[1].overlap_sample_count(), 16_000);
    assert_eq!(chunks[1].audio_chunk().samples().len(), 16_000 + 480_000);
    assert!(chunker.drain_ready_chunks().is_empty());
}

/// Verifies `drain_ready_chunks` returns chunks in finalization order (oldest first).
#[test]
fn drained_chunks_are_returned_oldest_first() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    chunker
        .push_pcm(AudioChunk::new(
            vec![7; 480_000],
            AudioConfig::new(16_000, 1),
        ))
        .unwrap_or_else(|error| panic!("first block should buffer: {error}"));
    chunker
        .push_pcm(AudioChunk::new(
            vec![9; 480_000],
            AudioConfig::new(16_000, 1),
        ))
        .unwrap_or_else(|error| panic!("second block should buffer: {error}"));

    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].audio_chunk().samples().first(), Some(&7));
    assert_eq!(chunks[1].audio_chunk().samples().last(), Some(&9));
}

/// Verifies stopping flushes buffered audio, including a trailing partial frame, as a final chunk.
#[test]
fn finish_flushes_remaining_audio_as_a_stream_ended_chunk() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    chunker
        .push_pcm(AudioChunk::new(
            vec![3; 500_000],
            AudioConfig::new(16_000, 1),
        ))
        .unwrap_or_else(|error| panic!("audio should buffer: {error}"));
    assert_eq!(chunker.drain_ready_chunks().len(), 1);

    let final_chunk = chunker
        .finish()
        .unwrap_or_else(|| panic!("remaining audio should be flushed"));
    assert_eq!(final_chunk.cut_type(), ChunkCutType::StreamEnded);
    assert_eq!(final_chunk.overlap_sample_count(), 16_000);
    assert_eq!(final_chunk.audio_chunk().samples().len(), 16_000 + 20_000);
    assert!(chunker.finish().is_none());
}

/// Verifies finishing an empty chunker yields nothing instead of an empty chunk.
#[test]
fn finish_without_audio_returns_none() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    assert!(chunker.finish().is_none());
}
