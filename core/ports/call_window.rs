//! Declares how the desktop reads who is speaking from a call app's window (#113).
//!
//! An adapter walks the accessibility tree of call windows only (Linux AT-SPI, Windows UI
//! Automation), reads element labels, and returns the names that `domain::speaker_names`
//! recognizes as speaking. It only reads: nothing is clicked, focused or changed. Labels and names
//! are personal data and are never logged.

/// Why a reading failed.
#[derive(Debug, thiserror::Error)]
pub enum CallWindowError {
    /// The platform's accessibility service could not be reached or answered with an error.
    #[error("accessibility service unavailable: {reason}")]
    Unavailable {
        /// A description without labels or names.
        reason: String,
    },
}

/// Reads the names call windows show as speaking right now.
pub trait CallWindowReader: Send {
    /// Takes one reading.
    ///
    /// Inputs: None. Outputs: The distinct names shown as speaking in call windows (empty when
    /// no call window is open or nobody is shown speaking), or an error when the accessibility
    /// service cannot be used. Side effects: Reads other apps' accessibility trees, within the
    /// adapter's node and time budget.
    fn speaking_names(&mut self) -> Result<Vec<String>, CallWindowError>;
}

/// Stores the names read during a recording with its meeting.
pub trait SpeakerNamesSink: Send {
    /// Attaches one recording's readings to its meeting.
    ///
    /// Inputs: The recorded meeting, its readings as WebVTT with one `<v Name>` cue per span
    /// (`domain::speaker_names::SpeakingTimeline::to_vtt`; personal data, never logged), and the
    /// meeting time the recording started at.
    /// Outputs: Success, or the storage failure. Side effects: Writes a sealed file and an
    /// attachment row.
    fn attach_speaker_names(
        &self,
        meeting_id: &crate::domain::models::MeetingId,
        webvtt: &str,
        meeting_time_ms: u64,
    ) -> Result<(), crate::domain::errors::StorageError>;
}
