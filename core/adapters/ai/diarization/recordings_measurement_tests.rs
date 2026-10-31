//! Measures transcription and diarization on real recordings (#61). All `#[ignore]`d: they need
//! recordings and model files. The diarization output holds voice embeddings, which are biometric
//! PII, and the transcription output holds what people said, so all output must stay local.
//!
//! `tools/diarization/evaluate_pipeline.py` documents the full run and turns the output into
//! aggregate figures. Common inputs: `NOTE_TAKER_MEASURE_WAV_DIR` (16 kHz mono PCM16 WAVs) and
//! `NOTE_TAKER_MEASURE_OUTPUT`.

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use serde_json::{Value, json};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
    WhisperVadParams,
};

use super::{
    onnx_models::{OnnxSegmentationModel, OnnxSpeakerEmbeddingModel},
    powerset::{LOCAL_SPEAKERS, active_speakers},
    wav_input::{SAMPLE_RATE_HZ, decode_wav_pcm16_mono},
    windowed_engine::{
        DiarizationTuning, SegmentationModel, SpeakerEmbeddingModel, WINDOW_SAMPLES,
        WindowedDiarizationEngine,
    },
};
use crate::{
    adapters::audio::vad_chunker::{VadChunker, VadConfig},
    domain::models::{AudioChunk, AudioConfig},
};

/// Samples per millisecond at the pipeline's rate.
const SAMPLES_PER_MS: usize = SAMPLE_RATE_HZ as usize / 1_000;
/// Least speech a local speaker needs to be embedded, as in `DiarizationTuning::default`.
const MIN_EMBEDDING_SAMPLES: usize = 400 * SAMPLES_PER_MS;

/// Reads a required path from the environment.
fn path_from_environment(variable: &str) -> PathBuf {
    PathBuf::from(env::var(variable).unwrap_or_else(|_| panic!("set {variable}")))
}

/// Lists the recordings, each with its name and decoded samples, in name order.
fn recordings() -> Vec<(String, Vec<f32>)> {
    let mut paths = fs::read_dir(path_from_environment("NOTE_TAKER_MEASURE_WAV_DIR"))
        .unwrap_or_else(|error| panic!("the recordings folder must be readable: {error}"))
        .map(|entry| entry.unwrap_or_else(|error| panic!("{error}")).path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .iter()
        .map(|path: &PathBuf| {
            let bytes = fs::read(path).unwrap_or_else(|error| panic!("{error}"));
            let samples = decode_wav_pcm16_mono(&bytes)
                .unwrap_or_else(|error| panic!("the recording must decode: {error}"));
            (stem(path), samples)
        })
        .collect()
}

/// Returns a path's file stem.
fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Cuts samples with the recorder's own chunker. Returns each chunk's start and length in samples.
///
/// `english` is the configuration every recording uses today (`pipeline_composition`).
fn vad_chunk_bounds(samples: &[f32]) -> Vec<(usize, usize)> {
    let mut chunker = VadChunker::new(VadConfig::english())
        .unwrap_or_else(|error| panic!("the chunker must start: {error}"));
    let pcm = samples
        .iter()
        .map(|sample| (sample * f32::from(i16::MAX)) as i16)
        .collect::<Vec<_>>();
    for block in pcm.chunks(1_600) {
        chunker
            .push_pcm(AudioChunk::new(
                block.to_vec(),
                AudioConfig::new(SAMPLE_RATE_HZ, 1),
            ))
            .unwrap_or_else(|error| panic!("the chunker must take audio: {error}"));
    }
    let mut chunks = chunker.drain_ready_chunks();
    chunks.extend(chunker.finish());
    let mut bounds = Vec::new();
    let mut next_start = 0;
    for chunk in chunks {
        let length = chunk.audio_chunk().samples().len();
        // Silent chunks still advance the timeline but are never transcribed or diarized.
        if chunk.contains_speech() {
            bounds.push((next_start, length));
        }
        next_start += length;
    }
    bounds
}

/// Collects a finished run's words, in milliseconds from `offset_ms`, with detected language.
fn words_from_state(state: &WhisperState, offset_ms: u64) -> Vec<Value> {
    let mut words: Vec<Value> = Vec::new();
    for segment in state.as_iter() {
        for token_index in 0..segment.n_tokens() {
            let Some(token) = segment.get_token(token_index) else {
                continue;
            };
            let text = token
                .to_str_lossy()
                .map(|t| t.into_owned())
                .unwrap_or_default();
            if text.starts_with("[_") || text.starts_with("<|") {
                continue;
            }
            let data = token.token_data();
            let (start_ms, end_ms) = (
                offset_ms + (data.t0.max(0) as u64) * 10,
                offset_ms + (data.t1.max(0) as u64) * 10,
            );
            let continues_word = !text.starts_with(' ') && !words.is_empty();
            if continues_word && let Some(last) = words.last_mut() {
                let joined = format!("{}{}", last["text"].as_str().unwrap_or(""), text);
                last["text"] = json!(joined);
                last["end_ms"] = json!(end_ms);
            } else {
                words.push(json!({"text": text.trim(), "start_ms": start_ms, "end_ms": end_ms}));
            }
        }
    }
    words
}

/// Writes one JSON line per recording: timed words under one transcription configuration.
///
/// Inputs: Common variables, plus `NOTE_TAKER_WHISPER_MODEL`, `MEASURE_MODE` (`whole` or `vad`),
/// `MEASURE_LANGUAGE` (`it` or `auto`), `MEASURE_BEAM` (0 for greedy) and optionally
/// `MEASURE_SILERO_MODEL` and `MEASURE_GPU_DEVICE`. Silero has no effect through whisper-rs:
/// `WhisperState::full` calls `whisper_full_with_state`, and only `whisper_full` applies VAD.
/// Outputs: Panics on failure, since a partial measurement must not pass for a full one.
/// Side effects: Runs inference and writes `NOTE_TAKER_MEASURE_OUTPUT`.
#[test]
#[ignore = "needs recordings and models; see tools/diarization/evaluate_pipeline.py"]
fn measure_transcription() {
    let mode = env::var("MEASURE_MODE").unwrap_or_else(|_| panic!("set MEASURE_MODE"));
    let language = env::var("MEASURE_LANGUAGE").unwrap_or_else(|_| panic!("set MEASURE_LANGUAGE"));
    let beam = env::var("MEASURE_BEAM")
        .unwrap_or_else(|_| panic!("set MEASURE_BEAM"))
        .parse::<i32>()
        .unwrap_or_else(|error| panic!("{error}"));
    let silero = env::var("MEASURE_SILERO_MODEL").ok();
    let model_path = path_from_environment("NOTE_TAKER_WHISPER_MODEL");
    let mut context_parameters = WhisperContextParameters::default();
    if let Some(device) = env::var("MEASURE_GPU_DEVICE")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        context_parameters.gpu_device = device;
    }
    let context = WhisperContext::new_with_params(
        model_path.to_str().unwrap_or_else(|| panic!("bad path")),
        context_parameters,
    )
    .unwrap_or_else(|error| panic!("the model must load: {error}"));

    let mut lines = Vec::new();
    for (name, samples) in recordings() {
        let pieces = if mode == "whole" {
            vec![(0, samples.len())]
        } else {
            vad_chunk_bounds(&samples)
        };
        let started = Instant::now();
        let mut words = Vec::new();
        let mut languages = Vec::new();
        for (start, length) in &pieces {
            let mut state = context
                .create_state()
                .unwrap_or_else(|error| panic!("{error}"));
            let strategy = if beam > 0 {
                SamplingStrategy::BeamSearch {
                    beam_size: beam,
                    patience: -1.0,
                }
            } else {
                SamplingStrategy::Greedy { best_of: 1 }
            };
            let mut parameters = FullParams::new(strategy);
            parameters.set_n_threads(8);
            parameters.set_print_progress(false);
            parameters.set_print_realtime(false);
            parameters.set_print_timestamps(false);
            parameters.set_token_timestamps(true);
            parameters.set_language(Some(language.as_str()));
            if let Some(silero_path) = &silero {
                parameters.set_vad_model_path(Some(silero_path.as_str()));
                parameters.set_vad_params(WhisperVadParams::default());
                parameters.enable_vad(true);
            }
            state
                .full(parameters, &samples[*start..*start + *length])
                .unwrap_or_else(|error| panic!("transcription must run: {error}"));
            languages.push(state.full_lang_id_from_state());
            words.extend(words_from_state(&state, (*start / SAMPLES_PER_MS) as u64));
        }
        lines.push(
            json!({
                "name": name,
                "duration_ms": (samples.len() / SAMPLES_PER_MS) as u64,
                "elapsed_ms": started.elapsed().as_millis() as u64,
                "pieces": pieces.iter().map(|(start, length)| json!({
                    "start_ms": start / SAMPLES_PER_MS,
                    "end_ms": (start + length) / SAMPLES_PER_MS,
                })).collect::<Vec<_>>(),
                "language_ids": languages,
                "words": words,
            })
            .to_string(),
        );
    }
    fs::write(
        path_from_environment("NOTE_TAKER_MEASURE_OUTPUT"),
        lines.join("\n"),
    )
    .unwrap_or_else(|error| panic!("the output must be written: {error}"));
}

/// Writes one JSON line per recording with diarization output for three designs:
/// `vad_turns` (today: the real chunks diarized one at a time), `whole_turns` (today's engine over
/// the whole recording), and `windows` (raw segmentation and embeddings on a sliding window, so the
/// scorer can try clustering over the whole meeting).
///
/// Inputs: Common variables, `MEASURE_WINDOW_STEP_MS`, and the two diarization model variables.
/// Outputs: Panics on failure. Side effects: Runs inference and writes embeddings to the output.
#[test]
#[ignore = "needs recordings and models; see tools/diarization/evaluate_pipeline.py"]
fn measure_diarization() {
    let step_samples = env::var("MEASURE_WINDOW_STEP_MS")
        .unwrap_or_else(|_| panic!("set MEASURE_WINDOW_STEP_MS"))
        .parse::<usize>()
        .unwrap_or_else(|error| panic!("{error}"))
        * SAMPLES_PER_MS;
    let load_engine = || {
        WindowedDiarizationEngine::new(
            OnnxSegmentationModel::load(&path_from_environment(
                "NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL",
            ))
            .unwrap_or_else(|error| panic!("segmentation must load: {error}")),
            OnnxSpeakerEmbeddingModel::load(&path_from_environment(
                "NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL",
            ))
            .unwrap_or_else(|error| panic!("embedding must load: {error}")),
            DiarizationTuning::default(),
        )
    };
    let engine = load_engine();
    let segmentation = OnnxSegmentationModel::load(&path_from_environment(
        "NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL",
    ))
    .unwrap_or_else(|error| panic!("segmentation must load: {error}"));

    let turns_of = |samples: &[f32], offset_ms: u64, chunk: usize| -> Vec<Value> {
        engine
            .diarize_samples(samples)
            .unwrap_or_else(|error| panic!("diarization must run: {error}"))
            .into_iter()
            .map(|segment| {
                json!({
                    "chunk": chunk,
                    "start_ms": offset_ms + segment.start_time_ms(),
                    "end_ms": offset_ms + segment.end_time_ms(),
                    "embedding": segment.embedding(),
                })
            })
            .collect()
    };

    let mut lines = Vec::new();
    for (name, samples) in recordings() {
        let chunks = vad_chunk_bounds(&samples);
        let vad_started = Instant::now();
        let vad_turns = chunks
            .iter()
            .enumerate()
            .flat_map(|(index, (start, length))| {
                turns_of(
                    &samples[*start..*start + *length],
                    (*start / SAMPLES_PER_MS) as u64,
                    index,
                )
            })
            .collect::<Vec<_>>();
        let vad_elapsed_ms = vad_started.elapsed().as_millis() as u64;
        let whole_turns = turns_of(&samples, 0, 0);

        let windows_started = Instant::now();
        let mut windows = Vec::new();
        let mut start = 0;
        loop {
            let end = (start + WINDOW_SAMPLES).min(samples.len());
            let mut window = samples[start..end].to_vec();
            window.resize(WINDOW_SAMPLES, 0.0);
            let scores = segmentation
                .segment(&window)
                .unwrap_or_else(|error| panic!("segmentation must run: {error}"));
            let frame_samples = WINDOW_SAMPLES as f64 / scores.len() as f64;
            let frames = scores.iter().map(active_speakers).collect::<Vec<_>>();
            let mut speakers = Vec::new();
            for local in 0..LOCAL_SPEAKERS {
                let speech = frames
                    .iter()
                    .enumerate()
                    .filter(|(frame, active)| {
                        active[local]
                            && active.iter().filter(|on| **on).count() == 1
                            && ((*frame as f64 * frame_samples) as usize) < end - start
                    })
                    .flat_map(|(frame, _)| {
                        let from = (frame as f64 * frame_samples) as usize;
                        let to = (((frame + 1) as f64 * frame_samples) as usize).min(end - start);
                        window[from..to].iter().copied()
                    })
                    .collect::<Vec<_>>();
                if speech.len() >= MIN_EMBEDDING_SAMPLES {
                    let embedding = engine
                        .embedding_model()
                        .embed(&speech)
                        .unwrap_or_else(|error| panic!("embedding must run: {error}"));
                    speakers.push(json!({
                        "local": local,
                        "speech_ms": speech.len() / SAMPLES_PER_MS,
                        "embedding": embedding,
                    }));
                }
            }
            let bitmask = frames
                .iter()
                .map(|active| {
                    active
                        .iter()
                        .enumerate()
                        .fold(0_u8, |bits, (index, on)| bits | (u8::from(*on) << index))
                })
                .collect::<Vec<_>>();
            windows.push(json!({
                "start_ms": start / SAMPLES_PER_MS,
                "length_ms": (end - start) / SAMPLES_PER_MS,
                "frame_ms": frame_samples / SAMPLES_PER_MS as f64,
                "frames": bitmask,
                "speakers": speakers,
            }));
            if end == samples.len() {
                break;
            }
            start += step_samples;
        }
        lines.push(
            json!({
                "name": name,
                "duration_ms": (samples.len() / SAMPLES_PER_MS) as u64,
                "vad_elapsed_ms": vad_elapsed_ms,
                "windows_elapsed_ms": windows_started.elapsed().as_millis() as u64,
                "vad_chunks": chunks.iter().map(|(start, length)| json!({
                    "start_ms": start / SAMPLES_PER_MS,
                    "end_ms": (start + length) / SAMPLES_PER_MS,
                })).collect::<Vec<_>>(),
                "vad_turns": vad_turns,
                "whole_turns": whole_turns,
                "windows": windows,
            })
            .to_string(),
        );
    }
    fs::write(
        path_from_environment("NOTE_TAKER_MEASURE_OUTPUT"),
        lines.join("\n"),
    )
    .unwrap_or_else(|error| panic!("the output must be written: {error}"));
}
