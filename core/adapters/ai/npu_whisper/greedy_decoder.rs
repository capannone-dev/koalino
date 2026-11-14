//! Whisper's greedy decoding over an NPU that encodes once and then steps token by token (#75).
//!
//! The prefix is an optional prompt (`<|startofprev|>` and the prompt's tokens), then
//! `<|startoftranscript|>`, the language and `<|transcribe|>`, with `<|notimestamps|>` when
//! timestamps are off. Without a language the model's choice after `<|startoftranscript|>` alone
//! is taken, as Whisper does; with a prompt, the decoder is then restarted so the prompt comes
//! first. Each later step takes the most likely allowed token: text, the end of text, and with
//! timestamps on, timestamps under Whisper's rules (a window starts with one, no later than 1 s;
//! they come in pairs between segments and never go back; when timestamps together are likelier
//! than any text token, one is taken). Other special tokens are never picked. The decoder's
//! window caps the length.
//!
//! `segments_of` turns the tokens into timed segments and says how much of the window they
//! covered, so the next window starts after the last finished segment, and `token_times` shares
//! a segment's time out among its tokens by length, for word timing.

use super::log_mel::{N_FRAMES, N_MELS};

/// `<|endoftext|>`.
pub const EOT: u32 = 50_257;
/// `<|startoftranscript|>`.
pub const SOT: u32 = 50_258;
/// `<|transcribe|>`.
const TRANSCRIBE: u32 = 50_359;
/// `<|startofprev|>`, which opens a prompt.
pub const SOT_PREV: u32 = 50_361;
/// `<|nospeech|>`, likely after `<|startoftranscript|>` when a window holds no speech.
pub const NO_SPEECH: u32 = 50_362;
/// `<|notimestamps|>`.
const NO_TIMESTAMPS: u32 = 50_363;
/// `<|0.00|>`, the first timestamp; each next one is 20 ms later, up to `<|30.00|>`.
pub const TIMESTAMP_BEGIN: u32 = 50_364;
/// How many timestamp tokens there are (0.00 s to 30.00 s).
const TIMESTAMP_COUNT: u32 = 1_501;
/// Milliseconds per timestamp step.
const MILLISECONDS_PER_TIMESTAMP: u32 = 20;
/// A window's first timestamp is no later than 1 s (50 steps), as in Whisper.
const MAX_INITIAL_TIMESTAMP: u32 = 50;
/// Samples per millisecond at 16 kHz.
const SAMPLES_PER_MILLISECOND: usize = 16;
/// The first language token (`<|en|>`).
const FIRST_LANGUAGE: u32 = 50_259;

/// Whisper's languages in token order (the multilingual models before large-v3).
const LANGUAGES: [&str; 99] = [
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su",
];

/// Why the NPU could not run a step.
#[derive(Debug, thiserror::Error)]
#[error("the NPU failed: {0}")]
pub struct NpuError(pub String);

/// An encoder and a stepping decoder.
pub trait WhisperNpu {
    /// Encodes a quantized 80 x 3,000 log-mel and resets the decoder's state.
    fn encode(&mut self, mel: &[u16]) -> Result<(), NpuError>;

    /// Forgets the tokens fed since the last encoding, keeping the encoded audio.
    fn restart(&mut self) -> Result<(), NpuError>;

    /// Feeds one token at `position` and returns the next token's (quantized) logits.
    fn step(&mut self, token: u32, position: usize) -> Result<Vec<u16>, NpuError>;
}

/// How the decoder's quantized logits map to real ones: `(q - zero_point) * scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogitScale {
    /// The step between two quantized values.
    pub scale: f32,
    /// The quantized value of 0.
    pub zero_point: i32,
}

/// What to decode.
#[derive(Clone, Copy, Debug)]
pub struct DecodeRequest<'a> {
    /// The language code; `None` or an unknown code lets the model choose.
    pub language: Option<&'a str>,
    /// Earlier text and the glossary, as tokens; empty for none.
    pub prompt: &'a [u32],
    /// Whether to decode timestamps (segment timing) as well.
    pub timestamps: bool,
    /// The most positions the decoder can take.
    pub max_positions: usize,
    /// How to read the logits as probabilities.
    pub logit_scale: LogitScale,
}

/// One decoded token and how sure the model was of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecodedToken {
    /// The token: text or a timestamp.
    pub token: u32,
    /// Its probability among all tokens, in [0, 1].
    pub probability: f32,
}

/// What a window decoded to.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// The text and timestamp tokens, without the prefix and the end of text.
    pub tokens: Vec<DecodedToken>,
    /// The language used, forced or chosen.
    pub language: &'static str,
    /// Whether decoding reached the end of text (`false`: the decoder's window was full).
    pub complete: bool,
    /// How likely `<|nospeech|>` was right after `<|startoftranscript|>`, in [0, 1].
    pub no_speech_probability: f32,
}

/// One timed stretch of speech in a window.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// Start, in milliseconds from the window's start.
    pub start_ms: u32,
    /// End, in milliseconds from the window's start.
    pub end_ms: u32,
    /// Its text tokens.
    pub tokens: Vec<DecodedToken>,
}

/// The token of a language code such as `it`; `None` for codes Whisper does not know.
pub fn language_token(code: &str) -> Option<u32> {
    LANGUAGES
        .iter()
        .position(|known| *known == code)
        .and_then(|index| u32::try_from(index).ok())
        .map(|index| FIRST_LANGUAGE + index)
}

/// Decodes one window greedily.
///
/// Inputs: The NPU, the quantized log-mel and the request. Outputs: The tokens, the language and
/// whether the end of text was reached, or the NPU's error. Side effects: Runs the NPU.
pub fn decode_greedy(
    npu: &mut dyn WhisperNpu,
    mel: &[u16],
    request: &DecodeRequest<'_>,
) -> Result<Decoded, NpuError> {
    debug_assert_eq!(mel.len(), N_MELS * N_FRAMES);
    npu.encode(mel)?;
    let max_positions = request.max_positions;
    let forced = request.language.and_then(language_token);
    let mut no_speech_probability = 0.0;
    let (language, mut position, mut logits) = match forced {
        Some(token) => (token, 0, Vec::new()),
        None => {
            let first = npu.step(SOT, 0)?;
            no_speech_probability =
                probability_of(&dequantize(&first, request.logit_scale), NO_SPEECH);
            let detected = best_in(&first, FIRST_LANGUAGE as usize, LANGUAGES.len())
                .and_then(|token| u32::try_from(token).ok())
                .unwrap_or(FIRST_LANGUAGE);
            if request.prompt.is_empty() {
                // SOT already sits at position 0; go on from there.
                (detected, 1, first)
            } else {
                npu.restart()?;
                (detected, 0, Vec::new())
            }
        }
    };
    let mut prefix = Vec::new();
    if position == 0 {
        if !request.prompt.is_empty() {
            prefix.push(SOT_PREV);
            prefix.extend_from_slice(request.prompt);
        }
        prefix.push(SOT);
    }
    prefix.extend([language, TRANSCRIBE]);
    if !request.timestamps {
        prefix.push(NO_TIMESTAMPS);
    }
    for token in prefix {
        if position >= max_positions {
            break;
        }
        logits = npu.step(token, position)?;
        if token == SOT {
            no_speech_probability =
                probability_of(&dequantize(&logits, request.logit_scale), NO_SPEECH);
        }
        position += 1;
    }
    let mut tokens: Vec<DecodedToken> = Vec::new();
    let mut complete = false;
    while position < max_positions && !logits.is_empty() {
        let real = dequantize(&logits, request.logit_scale);
        let allowed = allowed_tokens(&tokens, request.timestamps, &real);
        let Some(next) = most_likely(&real, &allowed) else {
            break;
        };
        if next == EOT {
            complete = true;
            break;
        }
        tokens.push(DecodedToken {
            token: next,
            probability: probability_of(&real, next),
        });
        logits = npu.step(next, position)?;
        position += 1;
    }
    let language_index = (language - FIRST_LANGUAGE) as usize;
    Ok(Decoded {
        tokens,
        language: LANGUAGES.get(language_index).copied().unwrap_or("en"),
        complete,
        no_speech_probability,
    })
}

/// Which tokens may come next: ranges of token IDs, half-open.
#[derive(Clone, Debug, Default)]
struct Allowed {
    /// Text tokens (before the end of text).
    text: bool,
    /// The end of text.
    end: bool,
    /// The allowed timestamp steps, half-open; empty for none.
    timestamps: std::ops::Range<u32>,
}

/// Applies Whisper's rules for the next token.
///
/// Inputs: The tokens sampled so far, whether timestamps are on, and the real logits.
/// Side effects: None.
fn allowed_tokens(sampled: &[DecodedToken], timestamps: bool, logits: &[f32]) -> Allowed {
    if !timestamps {
        return Allowed {
            text: true,
            end: true,
            timestamps: 0..0,
        };
    }
    let is_timestamp = |token: &DecodedToken| token.token >= TIMESTAMP_BEGIN;
    if sampled.is_empty() {
        return Allowed {
            text: false,
            end: false,
            timestamps: 0..MAX_INITIAL_TIMESTAMP + 1,
        };
    }
    let last_was_timestamp = sampled.last().is_some_and(is_timestamp);
    let penultimate_was_timestamp = sampled.len() < 2 || is_timestamp(&sampled[sampled.len() - 2]);
    let mut allowed = Allowed {
        text: true,
        end: true,
        timestamps: 0..TIMESTAMP_COUNT,
    };
    if last_was_timestamp {
        if penultimate_was_timestamp {
            allowed.timestamps = 0..0;
        } else {
            allowed.text = false;
        }
    }
    if let Some(last) = sampled.iter().rev().find(|token| is_timestamp(token)) {
        let last_step = last.token - TIMESTAMP_BEGIN;
        let earliest = if last_was_timestamp && !penultimate_was_timestamp {
            last_step
        } else {
            last_step + 1
        };
        allowed.timestamps = allowed.timestamps.start.max(earliest)..allowed.timestamps.end;
    }
    // When timestamps together are likelier than any single text token, take a timestamp.
    if allowed.text && !allowed.timestamps.is_empty() {
        let maximum = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let range = (TIMESTAMP_BEGIN + allowed.timestamps.start) as usize
            ..(TIMESTAMP_BEGIN + allowed.timestamps.end) as usize;
        let timestamp_mass: f32 = logits
            .get(range)
            .unwrap_or(&[])
            .iter()
            .map(|logit| (logit - maximum).exp())
            .sum();
        let best_text = logits
            .get(..=EOT as usize)
            .unwrap_or(&[])
            .iter()
            .map(|logit| (logit - maximum).exp())
            .fold(0.0_f32, f32::max);
        if timestamp_mass > best_text {
            allowed.text = false;
        }
    }
    allowed
}

/// The allowed token with the highest logit; the first on a tie.
fn most_likely(logits: &[f32], allowed: &Allowed) -> Option<u32> {
    let mut candidates: Vec<std::ops::Range<usize>> = Vec::new();
    if allowed.text {
        candidates.push(0..EOT as usize);
    }
    if allowed.end {
        candidates.push(EOT as usize..EOT as usize + 1);
    }
    if !allowed.timestamps.is_empty() {
        candidates.push(
            (TIMESTAMP_BEGIN + allowed.timestamps.start) as usize
                ..(TIMESTAMP_BEGIN + allowed.timestamps.end) as usize,
        );
    }
    candidates
        .into_iter()
        .flatten()
        .filter(|index| *index < logits.len())
        .reduce(|best, index| {
            if logits[index] > logits[best] {
                index
            } else {
                best
            }
        })
        .and_then(|index| u32::try_from(index).ok())
}

/// Reads quantized logits as real ones.
fn dequantize(logits: &[u16], scale: LogitScale) -> Vec<f32> {
    logits
        .iter()
        .map(|value| (i32::from(*value) - scale.zero_point) as f32 * scale.scale)
        .collect()
}

/// A token's probability among all tokens (softmax).
fn probability_of(logits: &[f32], token: u32) -> f32 {
    let maximum = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let total: f32 = logits.iter().map(|logit| (logit - maximum).exp()).sum();
    logits
        .get(token as usize)
        .map_or(0.0, |logit| (logit - maximum).exp() / total)
}

/// The index of the highest value in `logits[start..start + count]`; the first on a tie.
fn best_in(logits: &[u16], start: usize, count: usize) -> Option<usize> {
    let end = (start + count).min(logits.len());
    (start..end).reduce(|best, index| {
        if logits[index] > logits[best] {
            index
        } else {
            best
        }
    })
}

/// Splits a window's tokens into timed segments, as Whisper does.
///
/// Inputs: The tokens, whether decoding reached the end of text, and the window's length in
/// samples. Outputs: The segments, and how many samples they covered: up to the last pair of
/// timestamps when decoding stopped inside a segment, else the whole window. Text with no
/// timestamp pair becomes one segment from its first timestamp (else the start) to its last one
/// after the text (else the window's end). Side effects: None.
pub fn segments_of(
    tokens: &[DecodedToken],
    complete: bool,
    window_samples: usize,
) -> (Vec<Segment>, usize) {
    let is_timestamp = |index: usize| tokens[index].token >= TIMESTAMP_BEGIN;
    let milliseconds =
        |index: usize| (tokens[index].token - TIMESTAMP_BEGIN) * MILLISECONDS_PER_TIMESTAMP;
    let window_ms = u32::try_from(window_samples / SAMPLES_PER_MILLISECOND).unwrap_or(u32::MAX);
    let pairs: Vec<usize> = (1..tokens.len())
        .filter(|index| is_timestamp(*index) && is_timestamp(index - 1))
        .collect();
    let single_ending =
        tokens.len() >= 2 && is_timestamp(tokens.len() - 1) && !is_timestamp(tokens.len() - 2);
    if pairs.is_empty() {
        let text: Vec<DecodedToken> = tokens
            .iter()
            .copied()
            .filter(|token| token.token < EOT)
            .collect();
        if text.is_empty() {
            return (Vec::new(), window_samples);
        }
        let first_text = tokens
            .iter()
            .position(|token| token.token < EOT)
            .unwrap_or(0);
        let start_ms = (0..first_text)
            .find(|index| is_timestamp(*index))
            .map_or(0, milliseconds);
        let last_text = tokens
            .iter()
            .rposition(|token| token.token < EOT)
            .unwrap_or(0);
        let end_ms = (last_text + 1..tokens.len())
            .rev()
            .find(|index| is_timestamp(*index))
            .map_or(window_ms, milliseconds);
        return (
            vec![Segment {
                start_ms,
                end_ms: end_ms.max(start_ms),
                tokens: text,
            }],
            window_samples,
        );
    }
    let mut boundaries = pairs.clone();
    if single_ending {
        boundaries.push(tokens.len());
    }
    let mut segments = Vec::new();
    let mut last = 0;
    for boundary in boundaries {
        let slice = &tokens[last..boundary];
        if slice.len() >= 2 {
            let start_ms = milliseconds(last);
            let end_ms = milliseconds(boundary - 1);
            let text: Vec<DecodedToken> = slice
                .iter()
                .copied()
                .filter(|token| token.token < EOT)
                .collect();
            if !text.is_empty() && tokens[last].token >= TIMESTAMP_BEGIN {
                segments.push(Segment {
                    start_ms,
                    end_ms: end_ms.max(start_ms),
                    tokens: text,
                });
            }
        }
        last = boundary;
    }
    let consumed = if single_ending || (complete && last == tokens.len()) {
        window_samples
    } else {
        let resume_ms = milliseconds(last - 1) as usize;
        (resume_ms * SAMPLES_PER_MILLISECOND).min(window_samples)
    };
    // A window that moved nothing on (a pair at 0.00) is skipped whole, so decoding ends.
    let consumed = if consumed == 0 {
        window_samples
    } else {
        consumed
    };
    (segments, consumed)
}

/// Shares a segment's time out among its tokens in proportion to their byte lengths.
///
/// Inputs: The tokens' bytes and the segment's start and end in milliseconds. Outputs: Each
/// token's start and end in 10 ms units (whisper.cpp's), for `words_from_tokens`.
/// Side effects: None.
pub fn token_times(tokens: &[&[u8]], start_ms: u32, end_ms: u32) -> Vec<(i64, i64)> {
    let total: usize = tokens.iter().map(|bytes| bytes.len()).sum();
    let duration = f64::from(end_ms.saturating_sub(start_ms));
    let mut before = 0_usize;
    tokens
        .iter()
        .map(|bytes| {
            let at = |count: usize| {
                let share = if total == 0 {
                    0.0
                } else {
                    count as f64 / total as f64
                };
                ((f64::from(start_ms) + duration * share) / 10.0).round() as i64
            };
            let start = at(before);
            before += bytes.len();
            (start, at(before))
        })
        .collect()
}
