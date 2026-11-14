//! Whisper's greedy decoding over an NPU that encodes once and then steps token by token (#75).
//!
//! The prompt is `<|startoftranscript|>`, the language, `<|transcribe|>` and
//! `<|notimestamps|>`. Without a language the model's choice after the first token is taken, as
//! Whisper does. Each later step takes the highest-rated text token or the end of text; special
//! and timestamp tokens are never picked, which also keeps the loop from wandering into them. The
//! decoder's window caps the length.

use super::log_mel::{N_FRAMES, N_MELS};

/// `<|endoftext|>`.
pub const EOT: u32 = 50_257;
/// `<|startoftranscript|>`.
pub const SOT: u32 = 50_258;
/// `<|transcribe|>`.
const TRANSCRIBE: u32 = 50_359;
/// `<|notimestamps|>`.
const NO_TIMESTAMPS: u32 = 50_363;
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

    /// Feeds one token at `position` and returns the next token's (quantized) logits.
    fn step(&mut self, token: u32, position: usize) -> Result<Vec<u16>, NpuError>;
}

/// What a window decoded to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The text tokens, without the prompt and the end of text.
    pub tokens: Vec<u32>,
    /// The language used, forced or chosen.
    pub language: &'static str,
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
/// Inputs: The NPU, the quantized log-mel, the language code (`None` or an unknown code: the
/// model chooses), and the most positions the decoder can take. Outputs: The tokens and the
/// language, or the NPU's error. Side effects: Runs the NPU.
pub fn decode_greedy(
    npu: &mut dyn WhisperNpu,
    mel: &[u16],
    language: Option<&str>,
    max_positions: usize,
) -> Result<Decoded, NpuError> {
    debug_assert_eq!(mel.len(), N_MELS * N_FRAMES);
    npu.encode(mel)?;
    let first = npu.step(SOT, 0)?;
    let language_index = match language.and_then(language_token) {
        Some(token) => (token - FIRST_LANGUAGE) as usize,
        None => best_in(&first, FIRST_LANGUAGE as usize, LANGUAGES.len())
            .map(|token| token - FIRST_LANGUAGE as usize)
            .unwrap_or(0),
    };
    let language_code = LANGUAGES[language_index];
    let prompt = [
        FIRST_LANGUAGE + language_index as u32,
        TRANSCRIBE,
        NO_TIMESTAMPS,
    ];
    let mut logits = first;
    let mut position = 1;
    for token in prompt {
        if position >= max_positions {
            break;
        }
        logits = npu.step(token, position)?;
        position += 1;
    }
    let mut tokens = Vec::new();
    while position < max_positions {
        // Text tokens and the end of text only.
        let next = best_in(&logits, 0, EOT as usize + 1).unwrap_or(EOT as usize) as u32;
        if next == EOT {
            break;
        }
        tokens.push(next);
        logits = npu.step(next, position)?;
        position += 1;
    }
    Ok(Decoded {
        tokens,
        language: language_code,
    })
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
