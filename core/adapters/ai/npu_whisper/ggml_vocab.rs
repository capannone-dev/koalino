//! Whisper's mel filters and vocabulary, read from the header of a whisper.cpp model (#75).
//!
//! The shipped `ggml-base.bin` holds both: its header has the 11 hyperparameters, the mel filters
//! (`n_mels` x `n_fft` floats) and the token table (count, then length and bytes per token). The
//! multilingual models share one vocabulary, so the NPU's `small` reads it from `base` and no
//! tokenizer has to be shipped or downloaded. Only the header is read, never the weights.
//!
//! `tokenize` turns prompt text into tokens as Whisper's own (tiktoken) tokenizer does: GPT-2's
//! split into words (letters, digits or other symbols, each with an optional leading space, and
//! English contractions), then byte-pair merges. The merge table is not in the file, but it does
//! not need to be: in Whisper's vocabulary a token's ID is its merge rank, so each step merges the
//! adjacent pair whose joined bytes form the lowest-ID token.

use std::{collections::HashMap, io::Read};

/// The whisper.cpp model magic, `ggml` as a little-endian `u32`.
const MAGIC: u32 = 0x6767_6d6c;
/// Hyperparameters after the magic.
const HYPERPARAMETERS: usize = 11;
/// Upper bounds that keep a damaged file from asking for huge allocations.
const MAX_FILTER_VALUES: usize = 1 << 20;
const MAX_TOKENS: usize = 1 << 17;
const MAX_TOKEN_BYTES: usize = 1 << 10;

/// Why a model's header could not be read.
#[derive(Debug, thiserror::Error)]
pub enum GgmlVocabError {
    /// The file does not start like a whisper.cpp model.
    #[error("not a whisper.cpp model")]
    NotAModel,
    /// The header is cut short or holds impossible sizes.
    #[error("the model header is damaged")]
    Damaged,
    /// The file could not be read.
    #[error("the model could not be read: {0}")]
    Io(#[from] std::io::Error),
}

/// The mel filters and the text tokens.
#[derive(Clone, Debug)]
pub struct WhisperVocab {
    filters: Vec<f32>,
    filter_shape: (usize, usize),
    tokens: Vec<Vec<u8>>,
    /// The model's vocabulary size (`n_vocab`, the first hyperparameter).
    vocabulary_size: u32,
    /// Each token's ID by its bytes, for `tokenize`.
    ids: HashMap<Vec<u8>, u32>,
}

/// Tokens from `<|endoftext|>` on are special and never come from text.
const FIRST_SPECIAL: u32 = 50_257;

/// The vocabulary size of multilingual Whisper models; English-only ones have one token fewer.
const MULTILINGUAL_VOCABULARY: u32 = 51_865;

impl WhisperVocab {
    /// Reads the header.
    ///
    /// Inputs: A reader at the start of a whisper.cpp model. Outputs: The filters and tokens, or
    /// `NotAModel` / `Damaged` / `Io`. Side effects: Reads the header bytes only.
    pub fn read(reader: &mut impl Read) -> Result<Self, GgmlVocabError> {
        if read_u32(reader)? != MAGIC {
            return Err(GgmlVocabError::NotAModel);
        }
        let vocabulary_size = read_u32(reader)?;
        for _ in 1..HYPERPARAMETERS {
            read_u32(reader)?;
        }
        let rows = read_size(reader)?;
        let columns = read_size(reader)?;
        let count = rows
            .checked_mul(columns)
            .filter(|count| *count <= MAX_FILTER_VALUES)
            .ok_or(GgmlVocabError::Damaged)?;
        let mut filters = Vec::with_capacity(count);
        for _ in 0..count {
            filters.push(f32::from_le_bytes(read_array(reader)?));
        }
        let token_count = read_size(reader)?;
        if token_count > MAX_TOKENS {
            return Err(GgmlVocabError::Damaged);
        }
        let mut tokens = Vec::with_capacity(token_count);
        for _ in 0..token_count {
            let length = read_size(reader)?;
            if length > MAX_TOKEN_BYTES {
                return Err(GgmlVocabError::Damaged);
            }
            let mut token = vec![0; length];
            reader.read_exact(&mut token).map_err(short)?;
            tokens.push(token);
        }
        Ok(Self::with_tokens(
            filters,
            (rows, columns),
            tokens,
            vocabulary_size,
        ))
    }

    /// Builds the vocabulary and its lookup table.
    fn with_tokens(
        filters: Vec<f32>,
        filter_shape: (usize, usize),
        tokens: Vec<Vec<u8>>,
        vocabulary_size: u32,
    ) -> Self {
        let ids = tokens
            .iter()
            .enumerate()
            .filter_map(|(id, bytes)| Some((bytes.clone(), u32::try_from(id).ok()?)))
            .filter(|(_, id)| *id < FIRST_SPECIAL)
            .collect();
        Self {
            filters,
            filter_shape,
            tokens,
            vocabulary_size,
            ids,
        }
    }

    /// A vocabulary of the given tokens only, for tests.
    #[cfg(test)]
    pub(crate) fn from_tokens(tokens: &[&str]) -> Self {
        let bytes: Vec<&[u8]> = tokens.iter().map(|token| token.as_bytes()).collect();
        Self::from_byte_tokens(&bytes)
    }

    /// A vocabulary of the given byte tokens only (which need not be whole characters), for tests.
    #[cfg(test)]
    pub(crate) fn from_byte_tokens(tokens: &[&[u8]]) -> Self {
        Self::with_tokens(
            vec![0.0; super::log_mel::N_MELS * 201],
            (super::log_mel::N_MELS, 201),
            tokens.iter().map(|token| token.to_vec()).collect(),
            0,
        )
    }

    /// Turns text into token IDs, as Whisper's tokenizer does.
    ///
    /// Inputs: The text. Outputs: Each word's byte-pair tokens in turn; bytes with no token are
    /// skipped (every byte has one in a real vocabulary). Side effects: None.
    pub fn tokenize(&self, text: &str) -> Vec<u32> {
        let mut ids = Vec::new();
        for word in gpt2_words(text) {
            let mut parts: Vec<Vec<u8>> = word.bytes().map(|byte| vec![byte]).collect();
            // Merge the adjacent pair whose joined bytes have the lowest rank, until none joins.
            loop {
                let best = parts
                    .windows(2)
                    .enumerate()
                    .filter_map(|(index, pair)| {
                        let joined = [pair[0].as_slice(), pair[1].as_slice()].concat();
                        self.ids.get(&joined).map(|rank| (*rank, index))
                    })
                    .min();
                let Some((_, index)) = best else {
                    break;
                };
                let right = parts.remove(index + 1);
                parts[index].extend(right);
            }
            ids.extend(parts.iter().filter_map(|part| self.ids.get(part).copied()));
        }
        ids
    }

    /// The mel filters, row-major.
    pub fn mel_filters(&self) -> &[f32] {
        &self.filters
    }

    /// The filters' rows (mel bands) and columns (frequency bins).
    pub fn filter_shape(&self) -> (usize, usize) {
        self.filter_shape
    }

    /// Reports whether the model is multilingual, so its token IDs match the NPU model's.
    pub fn is_multilingual(&self) -> bool {
        self.vocabulary_size >= MULTILINGUAL_VOCABULARY
    }

    /// A token's bytes; empty for special and unknown tokens.
    pub fn bytes(&self, token: u32) -> &[u8] {
        self.tokens.get(token as usize).map_or(&[], Vec::as_slice)
    }

    /// The text of token IDs: their bytes joined and read as UTF-8 (invalid bytes replaced).
    /// Special and unknown tokens are skipped.
    pub fn text(&self, tokens: &[u32]) -> String {
        let bytes: Vec<u8> = tokens
            .iter()
            .filter_map(|token| self.tokens.get(*token as usize))
            .flatten()
            .copied()
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// A cut-short file is a damaged header.
fn short(error: std::io::Error) -> GgmlVocabError {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        GgmlVocabError::Damaged
    } else {
        GgmlVocabError::Io(error)
    }
}

fn read_array(reader: &mut impl Read) -> Result<[u8; 4], GgmlVocabError> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes).map_err(short)?;
    Ok(bytes)
}

fn read_u32(reader: &mut impl Read) -> Result<u32, GgmlVocabError> {
    Ok(u32::from_le_bytes(read_array(reader)?))
}

fn read_size(reader: &mut impl Read) -> Result<usize, GgmlVocabError> {
    usize::try_from(read_u32(reader)?).map_err(|_| GgmlVocabError::Damaged)
}

/// What a character counts as in GPT-2's word split.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Letter,
    Digit,
    Space,
    Other,
}

/// Classifies a character as GPT-2's pattern does: letters are `\p{L}` only, so combining marks
/// (Thai and Devanagari vowel signs, for example) are `Other`, though Rust counts many as
/// alphabetic.
fn class_of(character: char) -> CharClass {
    if super::unicode_letters::is_letter(character) {
        CharClass::Letter
    } else if character.is_numeric() {
        CharClass::Digit
    } else if character.is_whitespace() {
        CharClass::Space
    } else {
        CharClass::Other
    }
}

/// Splits text like GPT-2's pattern: `'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+|
/// ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+`.
fn gpt2_words(text: &str) -> Vec<&str> {
    const CONTRACTIONS: [&str; 7] = ["'s", "'t", "'re", "'ve", "'m", "'ll", "'d"];
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    let byte_at = |index: usize| characters.get(index).map_or(text.len(), |(at, _)| *at);
    let mut words = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        let start = index;
        let rest = &text[byte_at(index)..];
        if let Some(contraction) = CONTRACTIONS.iter().find(|c| rest.starts_with(**c)) {
            index += contraction.chars().count();
            words.push(&text[byte_at(start)..byte_at(index)]);
            continue;
        }
        let class = class_of(characters[index].1);
        let next_class = characters.get(index + 1).map(|(_, c)| class_of(*c));
        if class == CharClass::Space {
            // A run of spaces; its last space joins the next word, unless the text ends.
            let mut end = index;
            while end < characters.len() && class_of(characters[end].1) == CharClass::Space {
                end += 1;
            }
            let keeps_last = end < characters.len() && characters[end - 1].1 == ' ';
            let run_end = if keeps_last { end - 1 } else { end };
            if run_end > index {
                words.push(&text[byte_at(index)..byte_at(run_end)]);
                index = run_end;
                continue;
            }
            // A single space before a word: it starts that word.
            let word_class = next_class.unwrap_or(CharClass::Space);
            index += 1;
            while index < characters.len() && class_of(characters[index].1) == word_class {
                index += 1;
            }
            words.push(&text[byte_at(start)..byte_at(index)]);
            continue;
        }
        while index < characters.len() && class_of(characters[index].1) == class {
            index += 1;
        }
        words.push(&text[byte_at(start)..byte_at(index)]);
    }
    words
}
