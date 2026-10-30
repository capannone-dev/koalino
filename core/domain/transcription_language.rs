//! Defines the language a recording is transcribed in (Issue #62): `Auto`, where Whisper detects
//! it per chunk, or one of the codes whisper.cpp knows. whisper.cpp fails every chunk on a code it
//! does not know, so only those can be built.

/// Every language code whisper.cpp accepts, in its own order.
pub const WHISPER_LANGUAGE_CODES: &[&str] = &[
    "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it",
    "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur",
    "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn",
    "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si",
    "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo",
    "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln",
    "ha", "ba", "jw", "su", "yue",
];

/// The text form of `TranscriptionLanguage::Auto`.
const AUTO_TEXT: &str = "auto";

/// A transcription language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptionLanguage {
    /// Whisper detects the language of each chunk.
    Auto,
    /// One of `WHISPER_LANGUAGE_CODES`.
    Code(&'static str),
}

/// Refuses text that is neither `auto` nor a Whisper language code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("not a transcription language")]
pub struct InvalidTranscriptionLanguage;

impl TranscriptionLanguage {
    /// Reads a language from text.
    ///
    /// Inputs: `auto` or a Whisper code, in any case and with surrounding whitespace.
    /// Outputs: The language, or `InvalidTranscriptionLanguage`. Side effects: None.
    pub fn parse(text: &str) -> Result<Self, InvalidTranscriptionLanguage> {
        let text = text.trim().to_ascii_lowercase();
        if text == AUTO_TEXT {
            return Ok(Self::Auto);
        }
        WHISPER_LANGUAGE_CODES
            .iter()
            .find(|code| **code == text)
            .map(|code| Self::Code(code))
            .ok_or(InvalidTranscriptionLanguage)
    }

    /// Returns the text form: `auto` or the Whisper code. Side effects: None.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => AUTO_TEXT,
            Self::Code(code) => code,
        }
    }
}
