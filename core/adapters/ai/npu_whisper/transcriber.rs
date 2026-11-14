//! Transcribes a recording on the NPU window by window (#75), as Whisper does.
//!
//! Each 30 s window is decoded with timestamps. The window's finished segments are kept, and the
//! next window starts where the last finished segment ended, so a word cut by the window's edge is
//! decoded again in the next one. Each window is prompted with the earlier text: the caller's
//! prompt (the chunk's previous text and the glossary), then the text decoded so far, keeping the
//! most recent tokens. A segment's time is shared out among its tokens by length, which gives each
//! word an approximate time and the probability of its least sure token. Segments that are only a
//! non-speech marker such as `[BLANK_AUDIO]` are dropped, and so is a whole window the model
//! thinks holds no speech and whose words it is unsure of (Whisper's no-speech rule), which keeps
//! made-up phrases out of silence.

use super::{
    ggml_vocab::WhisperVocab,
    greedy_decoder::{
        DecodeRequest, LogitScale, NpuError, WhisperNpu, decode_greedy, segments_of, token_times,
    },
    log_mel::{N_SAMPLES, log_mel, quantize},
};
use crate::{
    adapters::ai::whisper_word_timing::{
        WhisperTokenTiming, is_non_speech_marker, words_from_tokens, words_match_text,
    },
    domain::models::TimedWord,
};

/// The most prompt tokens a window takes. The decoder holds 200 positions; this leaves room for
/// a full window of speech after the prefix.
pub const MAX_PROMPT_TOKENS: usize = 64;

/// Windows shorter than this (0.1 s) are not decoded.
const MIN_WINDOW_SAMPLES: usize = 1_600;

/// Whisper's no-speech rule: a window is dropped when `<|nospeech|>` is likelier than this...
const NO_SPEECH_THRESHOLD: f32 = 0.6;
/// ...and its text tokens' mean log probability is below this.
const LOG_PROBABILITY_THRESHOLD: f32 = -1.0;

/// Samples per millisecond at 16 kHz.
const SAMPLES_PER_MILLISECOND: usize = 16;

/// How to transcribe.
#[derive(Clone, Debug)]
pub struct WindowSettings<'a> {
    /// The language code; `None` lets the model choose per window.
    pub language: Option<&'a str>,
    /// The caller's prompt as tokens (previous text and glossary); empty for none.
    pub prompt: Vec<u32>,
    /// The encoder input's quantization step.
    pub input_scale: f32,
    /// The encoder input's quantized zero.
    pub input_zero_point: i32,
    /// The decoder logits' quantization.
    pub logit_scale: LogitScale,
    /// The most positions the decoder takes.
    pub max_positions: usize,
}

/// A recording's text and timed words.
#[derive(Clone, Debug, PartialEq)]
pub struct NpuTranscript {
    /// The text, trimmed.
    pub text: String,
    /// Its words with times from the recording's start; empty when they would not match the text.
    pub words: Vec<TimedWord>,
}

/// Transcribes 16 kHz mono samples.
///
/// Inputs: The NPU, the vocabulary (with the mel filters), the samples and the settings.
/// Outputs: The text and timed words, or the NPU's error. Side effects: Runs the NPU.
pub fn transcribe_windows(
    npu: &mut dyn WhisperNpu,
    vocabulary: &WhisperVocab,
    samples: &[f32],
    settings: &WindowSettings<'_>,
) -> Result<NpuTranscript, NpuError> {
    let mut history = settings.prompt.clone();
    let mut text = String::new();
    // Each token's bytes, times (10 ms units) and probability, over the whole recording.
    let mut timed_tokens: Vec<(u32, i64, i64, f32)> = Vec::new();
    let mut offset = 0;
    while samples.len().saturating_sub(offset) >= MIN_WINDOW_SAMPLES {
        let window = &samples[offset..samples.len().min(offset + N_SAMPLES)];
        let mel = quantize(
            &log_mel(window, vocabulary.mel_filters()),
            settings.input_scale,
            settings.input_zero_point,
        );
        let prompt = &history[history.len().saturating_sub(MAX_PROMPT_TOKENS)..];
        let decoded = decode_greedy(
            npu,
            &mel,
            &DecodeRequest {
                language: settings.language,
                prompt,
                timestamps: true,
                max_positions: settings.max_positions,
                logit_scale: settings.logit_scale,
            },
        )?;
        let (segments, consumed) = segments_of(&decoded.tokens, decoded.complete, window.len());
        if seems_silent(&decoded) {
            offset += consumed.max(1);
            continue;
        }
        let offset_ms = u32::try_from(offset / SAMPLES_PER_MILLISECOND).unwrap_or(u32::MAX);
        for segment in segments {
            let ids: Vec<u32> = segment.tokens.iter().map(|token| token.token).collect();
            let segment_text = vocabulary.text(&ids);
            if is_non_speech_marker(&segment_text) {
                continue;
            }
            text.push_str(&segment_text);
            history.extend_from_slice(&ids);
            let bytes: Vec<&[u8]> = ids.iter().map(|id| vocabulary.bytes(*id)).collect();
            let times = token_times(
                &bytes,
                offset_ms.saturating_add(segment.start_ms),
                offset_ms.saturating_add(segment.end_ms),
            );
            for (token, (start, end)) in segment.tokens.iter().zip(times) {
                timed_tokens.push((token.token, start, end, token.probability));
            }
        }
        offset += consumed.max(1);
    }
    let words = words_from_tokens(timed_tokens.iter().map(|(token, start, end, probability)| {
        WhisperTokenTiming {
            bytes: vocabulary.bytes(*token),
            start_units: *start,
            end_units: *end,
            probability: *probability,
        }
    }));
    let text = text.trim().to_owned();
    let words = if words_match_text(&words, &text) {
        words
    } else {
        Vec::new()
    };
    Ok(NpuTranscript { text, words })
}

/// Whisper's no-speech rule: `<|nospeech|>` likely and the text unsure.
fn seems_silent(decoded: &super::greedy_decoder::Decoded) -> bool {
    let text: Vec<f32> = decoded
        .tokens
        .iter()
        .filter(|token| token.token < super::greedy_decoder::EOT)
        .map(|token| token.probability.max(f32::MIN_POSITIVE).ln())
        .collect();
    let mean = if text.is_empty() {
        0.0
    } else {
        text.iter().sum::<f32>() / text.len() as f32
    };
    decoded.no_speech_probability > NO_SPEECH_THRESHOLD && mean < LOG_PROBABILITY_THRESHOLD
}
