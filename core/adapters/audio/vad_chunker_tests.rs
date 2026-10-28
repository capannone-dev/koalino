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
    push_samples(&mut english_chunker, speech_samples(3_000));
    push_silent_frames(&mut english_chunker, VAD_HANGOVER_FRAMES + 19);
    assert!(english_chunker.drain_ready_chunks().is_empty());
    push_silent_frames(&mut english_chunker, 1);
    let english_chunk = english_chunker
        .next_chunk()
        .unwrap_or_else(|error| panic!("English chunk retrieval should work: {error}"))
        .unwrap_or_else(|| panic!("600 ms of silence must finalize an English VAD pause"));
    assert_eq!(english_chunk.cut_type(), ChunkCutType::VadPause);

    let mut italian_chunker = VadChunker::new(VadConfig::italian())
        .unwrap_or_else(|error| panic!("Italian configuration should work: {error}"));
    push_samples(&mut italian_chunker, speech_samples(3_000));
    push_silent_frames(&mut italian_chunker, VAD_HANGOVER_FRAMES + 26);
    assert!(italian_chunker.drain_ready_chunks().is_empty());
    push_silent_frames(&mut italian_chunker, 1);
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

/// Builds `duration_ms` of loud alternating PCM with the same energy in every frame.
fn loud_samples(duration_ms: usize) -> Vec<i16> {
    (0..duration_ms * 16)
        .map(|sample_index| if sample_index % 2 == 0 { 8_000 } else { -8_000 })
        .collect()
}

/// Builds `duration_ms` of synthetic speech: two tones at a syllable-rate envelope plus a little
/// noise. WebRTC keeps classifying it as voice for many seconds, unlike a constant square wave,
/// which its noise adaptation soon treats as background.
fn speech_samples(duration_ms: usize) -> Vec<i16> {
    let mut noise_state: u32 = 12_345;
    (0..duration_ms * 16)
        .map(|sample_index| {
            noise_state = noise_state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let noise = ((noise_state >> 16) % 2_000) as f32 / 2_000.0;
            let seconds = sample_index as f32 / 16_000.0;
            let envelope = 0.55 + 0.45 * (std::f32::consts::TAU * 4.0 * seconds).sin();
            let voiced = (std::f32::consts::TAU * 150.0 * seconds).sin() * 0.6
                + (std::f32::consts::TAU * 450.0 * seconds).sin() * 0.3
                + noise * 0.1;
            (voiced * envelope * 12_000.0) as i16
        })
        .collect()
}

/// Frames WebRTC still reports as voice after speech stops (its hangover); measured as six.
const VAD_HANGOVER_FRAMES: usize = 6;

/// Pushes `samples` in one block.
fn push_samples(chunker: &mut VadChunker, samples: Vec<i16>) {
    chunker
        .push_pcm(AudioChunk::new(samples, AudioConfig::new(16_000, 1)))
        .unwrap_or_else(|error| panic!("PCM should buffer: {error}"));
}

/// Verifies a forced cut lands at the end of the quietest frame of the last three seconds, and
/// the audio after it starts the next chunk instead of being lost or repeated (Issue #51).
#[test]
fn forced_cut_lands_on_the_quietest_recent_frame_without_overlap() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    let mut samples = loud_samples(16_000);
    // A near-silent frame starting at 13.5 s: the gap between two words.
    let quiet_start = 13_500 * 16;
    samples[quiet_start..quiet_start + VAD_FRAME_SAMPLES].fill(3);
    push_samples(&mut chunker, samples);

    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].cut_type(), ChunkCutType::SoftMaxForced);
    assert_eq!(
        chunks[0].audio_chunk().samples().len(),
        quiet_start + VAD_FRAME_SAMPLES
    );
    let rest = chunker
        .finish()
        .unwrap_or_else(|| panic!("the audio after the cut should remain"));
    assert_eq!(
        rest.audio_chunk().samples().len(),
        16_000 * 16 - quiet_start - VAD_FRAME_SAMPLES
    );
}

/// Verifies a quiet frame older than the search window does not pull the cut back.
#[test]
fn forced_cut_ignores_quiet_frames_before_the_search_window() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    let mut samples = loud_samples(15_000);
    samples[5_000 * 16..5_000 * 16 + VAD_FRAME_SAMPLES].fill(0);
    push_samples(&mut chunker, samples);

    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 1);
    // Every recent frame is equally loud, so the cut stays at the 15-second cap.
    assert_eq!(chunks[0].audio_chunk().samples().len(), 15_000 * 16);
}

/// Verifies chunking never drops or repeats audio: the chunks joined equal the input.
#[test]
fn chunks_joined_reproduce_the_input_exactly() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    // Distinct values everywhere, with a quiet dip every 4.1 s.
    let input: Vec<i16> = (0..50_000 * 16)
        .map(|sample_index: usize| {
            if (sample_index / 16) % 4_100 < 30 {
                1
            } else {
                1_000 + (sample_index % 20_000) as i16
            }
        })
        .collect();
    for block in input.chunks(7_777) {
        push_samples(&mut chunker, block.to_vec());
    }

    let mut chunks = chunker.drain_ready_chunks();
    chunks.extend(chunker.finish());
    assert!(chunks.len() >= 4);
    assert!(
        chunks
            .iter()
            .all(|chunk| chunk.audio_chunk().samples().len() <= 15_000 * 16)
    );
    let joined: Vec<i16> = chunks
        .iter()
        .flat_map(|chunk| chunk.audio_chunk().samples().to_vec())
        .collect();
    assert_eq!(joined, input);
}

/// Verifies VAD mode also caps continuous speech at 15 seconds, so the live transcript keeps
/// up (Issue #52).
#[test]
#[cfg(feature = "vad")]
fn continuous_speech_is_cut_within_fifteen_seconds() {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("English configuration should work: {error}"));
    push_samples(&mut chunker, speech_samples(16_000));

    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].cut_type(), ChunkCutType::SoftMaxForced);
    assert!(chunks[0].audio_chunk().samples().len() <= 15_000 * 16);
}

/// Verifies a pause does not cut a chunk shorter than three seconds, so short phrases are not
/// transcribed alone, and does cut once the chunk is long enough.
#[test]
#[cfg(feature = "vad")]
fn pause_cuts_only_chunks_of_at_least_three_seconds() {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("English configuration should work: {error}"));
    push_samples(&mut chunker, speech_samples(1_000));
    push_silent_frames(&mut chunker, 30);
    assert!(
        chunker.drain_ready_chunks().is_empty(),
        "1.9 s is too short to stand alone"
    );

    push_samples(&mut chunker, speech_samples(1_500));
    push_silent_frames(&mut chunker, VAD_HANGOVER_FRAMES + 20);
    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].cut_type(), ChunkCutType::VadPause);
}

/// Verifies a chunk VAD heard no speech in says so, so it is not sent to Whisper, which invents
/// text for silence (Issue #51), while a chunk with speech says it has some.
#[test]
#[cfg(feature = "vad")]
fn chunks_report_whether_they_contain_speech() {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("English configuration should work: {error}"));
    push_silent_frames(&mut chunker, 110);
    push_samples(&mut chunker, speech_samples(3_000));
    push_silent_frames(&mut chunker, VAD_HANGOVER_FRAMES + 20);

    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 2);
    assert!(!chunks[0].contains_speech(), "3.3 s of silence");
    assert!(chunks[1].contains_speech());
}

/// Verifies time-based chunks, which nothing classifies, are always treated as speech.
#[test]
fn time_based_chunks_are_treated_as_speech() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    push_silent_frames(&mut chunker, 600);

    let mut chunks = chunker.drain_ready_chunks();
    chunks.extend(chunker.finish());
    assert_eq!(chunks.len(), 2);
    assert!(chunks.iter().all(|chunk| chunk.contains_speech()));
}

/// Verifies disabled VAD uses bounded time chunking and records a warning instead of failing audio.
///
/// Inputs: Thirty seconds of 16 kHz mono PCM while the `vad` feature is absent.
/// Outputs: A time-forced chunk and source-level confirmation of the fallback warning contract.
/// Side effects: Buffers in-memory test PCM only; it does not access a microphone or log samples.
#[test]
#[cfg(not(feature = "vad"))]
fn disabled_vad_falls_back_to_fifteen_second_time_chunking_with_warn_log() {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("fallback configuration should work: {error}"));
    push_silent_frames(&mut chunker, 1_000);
    let fallback_chunk = chunker
        .next_chunk()
        .unwrap_or_else(|error| panic!("fallback chunk retrieval should work: {error}"))
        .unwrap_or_else(|| panic!("disabled VAD must finalize within 15 seconds"));

    assert_eq!(fallback_chunk.cut_type(), ChunkCutType::SoftMaxForced);
    let adapter_source = include_str!("vad_chunker.rs");
    assert!(
        adapter_source.contains("falling back to 15-second time-based chunking"),
        "disabled or unavailable VAD must emit a warning describing time fallback"
    );
}

/// Verifies runtime time-based mode cuts every 15 seconds even when the `vad` feature is built,
/// each chunk starting where the previous one ended.
#[test]
fn time_based_mode_cuts_every_fifteen_seconds_without_overlap() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    push_silent_frames(&mut chunker, 1_000);

    let chunks = chunker.drain_ready_chunks();
    assert_eq!(chunks.len(), 2);
    assert!(chunks.iter().all(|chunk| {
        chunk.cut_type() == ChunkCutType::SoftMaxForced
            && chunk.audio_chunk().samples().len() == 240_000
    }));
    assert!(chunker.drain_ready_chunks().is_empty());
}

/// Verifies `drain_ready_chunks` returns chunks in finalization order (oldest first).
#[test]
fn drained_chunks_are_returned_oldest_first() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    chunker
        .push_pcm(AudioChunk::new(
            vec![7; 240_000],
            AudioConfig::new(16_000, 1),
        ))
        .unwrap_or_else(|error| panic!("first block should buffer: {error}"));
    chunker
        .push_pcm(AudioChunk::new(
            vec![9; 240_000],
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
    assert_eq!(chunker.drain_ready_chunks().len(), 2);

    let final_chunk = chunker
        .finish()
        .unwrap_or_else(|| panic!("remaining audio should be flushed"));
    assert_eq!(final_chunk.cut_type(), ChunkCutType::StreamEnded);
    assert_eq!(final_chunk.audio_chunk().samples().len(), 500_000 - 480_000);
    assert!(chunker.finish().is_none());
}

/// Verifies finishing an empty chunker yields nothing instead of an empty chunk.
#[test]
fn finish_without_audio_returns_none() {
    let mut chunker = VadChunker::time_based(VadConfig::english());
    assert!(chunker.finish().is_none());
}
