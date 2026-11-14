//! Whisper's mel filters and vocabulary, read from the header of a whisper.cpp model (#75).
//!
//! The shipped `ggml-base.bin` holds both: its header has the 11 hyperparameters, the mel filters
//! (`n_mels` x `n_fft` floats) and the token table (count, then length and bytes per token). The
//! multilingual models share one vocabulary, so the NPU's `small` reads it from `base` and no
//! tokenizer has to be shipped or downloaded. Only the header is read, never the weights.

use std::io::Read;

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
}

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
        Ok(Self {
            filters,
            filter_shape: (rows, columns),
            tokens,
            vocabulary_size,
        })
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
