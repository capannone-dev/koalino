//! Defines immutable, strongly typed business values for meetings and recordings.

use crate::domain::transcription_language::TranscriptionLanguage;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::errors::{ExportError, MeetingTitleError};

/// Selects the portable representation written by a meeting export request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ExportFormat {
    /// Human-readable Markdown suitable for editors and source control.
    Markdown,
    /// Paginated Portable Document Format suitable for sharing and printing.
    Pdf,
    /// Versioned structured data suitable for external integrations.
    Json,
    /// Reserved SubRip subtitle output; parsing accepts it but no exporter implements it yet.
    Srt,
}

impl ExportFormat {
    /// Returns the conventional filename suffix for this format.
    ///
    /// Inputs: None.
    /// Outputs: A lowercase extension without a leading dot.
    /// Side effects: None.
    pub fn file_extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Pdf => "pdf",
            Self::Json => "json",
            Self::Srt => "srt",
        }
    }

    /// Returns the canonical lowercase name accepted by every driving interface.
    ///
    /// Inputs: None.
    /// Outputs: `markdown`, `pdf`, `json`, or `srt`.
    /// Side effects: None.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Pdf => "pdf",
            Self::Json => "json",
            Self::Srt => "srt",
        }
    }
}

impl std::fmt::Display for ExportFormat {
    /// Writes the canonical lowercase format name.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for ExportFormat {
    type Err = ExportError;

    /// Parses a case-insensitive format name (`markdown`/`md`, `pdf`, `json`, `srt`).
    ///
    /// Inputs: `value` is user-supplied text from a CLI flag, HTTP body, or FFI string.
    /// Outputs: The matching format or `ExportError::UnknownFormat`.
    /// Side effects: None.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => Ok(Self::Markdown),
            "pdf" => Ok(Self::Pdf),
            "json" => Ok(Self::Json),
            "srt" => Ok(Self::Srt),
            _ => Err(ExportError::UnknownFormat {
                value: String::from(value),
            }),
        }
    }
}

/// Narrows a batch export to meetings created in an inclusive UTC date range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportDateRange {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
}

impl ExportDateRange {
    /// Creates an inclusive export date range.
    ///
    /// Inputs: `start` and `end` are UTC instants.
    /// Outputs: The range, or `ExportError::InvalidDateRange` when `start` is after `end`.
    /// Side effects: None.
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self, ExportError> {
        if start > end {
            return Err(ExportError::InvalidDateRange);
        }
        Ok(Self { start, end })
    }

    /// Returns the inclusive UTC start instant.
    pub fn start(&self) -> DateTime<Utc> {
        self.start
    }

    /// Returns the inclusive UTC end instant.
    pub fn end(&self) -> DateTime<Utc> {
        self.end
    }

    /// Reports whether `timestamp` lies inside this inclusive range.
    ///
    /// Inputs: `timestamp` is a UTC instant such as a meeting creation time.
    /// Outputs: `true` when `start <= timestamp <= end`.
    /// Side effects: None.
    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        self.start <= timestamp && timestamp <= self.end
    }
}

/// Configures one meeting or batch export request without exposing transport-specific values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportOptions {
    format: ExportFormat,
    include_attachments: bool,
    include_metadata: bool,
    date_range: Option<ExportDateRange>,
}

impl ExportOptions {
    /// Creates export options with explicit inclusion policy and an optional batch date range.
    ///
    /// Inputs: `format` selects the exporter; the booleans control attachment and metadata
    /// sections; `date_range` filters batch selection and is ignored for explicit meeting IDs.
    /// Outputs: Immutable options.
    /// Side effects: None.
    pub fn new(
        format: ExportFormat,
        include_attachments: bool,
        include_metadata: bool,
        date_range: Option<ExportDateRange>,
    ) -> Self {
        Self {
            format,
            include_attachments,
            include_metadata,
            date_range,
        }
    }

    /// Returns defaults for one format: attachments and metadata included, no date filter.
    pub fn for_format(format: ExportFormat) -> Self {
        Self::new(format, true, true, None)
    }

    /// Returns the selected output format.
    pub fn format(&self) -> ExportFormat {
        self.format
    }

    /// Reports whether attachment references (and staged copies) belong in the output.
    pub fn include_attachments(&self) -> bool {
        self.include_attachments
    }

    /// Reports whether the meeting metadata header (date, duration, participants) belongs in output.
    pub fn include_metadata(&self) -> bool {
        self.include_metadata
    }

    /// Borrows the optional inclusive batch-selection range.
    pub fn date_range(&self) -> Option<&ExportDateRange> {
        self.date_range.as_ref()
    }
}

/// Reports the file created by an export adapter without including meeting text or attachment bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportResult {
    file_path: String,
    format: ExportFormat,
    size_bytes: u64,
    /// Safe key/value facts such as `schema_version` and `meeting_count`.
    metadata: Vec<(String, String)>,
}

impl ExportResult {
    /// Creates an immutable successful export summary.
    ///
    /// Inputs: `file_path` is the written artifact; `size_bytes` its final length; `metadata`
    /// contains display-safe facts only.
    /// Outputs: The summary.
    /// Side effects: None.
    pub fn new(
        file_path: impl Into<String>,
        format: ExportFormat,
        size_bytes: u64,
        metadata: Vec<(String, String)>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            format,
            size_bytes,
            metadata,
        }
    }

    /// Borrows the created file's local path.
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Returns the format used to create the result.
    pub fn format(&self) -> ExportFormat {
        self.format
    }

    /// Returns the final byte length observed after writing.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    /// Borrows safe result metadata, such as schema version or exported meeting count.
    pub fn metadata(&self) -> &[(String, String)] {
        &self.metadata
    }

    /// Looks up one metadata value by key.
    ///
    /// Inputs: `key` is a metadata name such as `meeting_count`.
    /// Outputs: The first matching value, if present.
    /// Side effects: None.
    pub fn metadata_value(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|(metadata_key, _)| metadata_key == key)
            .map(|(_, metadata_value)| metadata_value.as_str())
    }
}

/// Represents attachment metadata prepared for rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportAttachment {
    attachment_id: Uuid,
    /// Original repository path of the attachment; never modified by export.
    file_path: String,
    mime_type: String,
    original_timestamp: DateTime<Utc>,
    /// Export-owned copy of the file, set by the application layer when staging succeeded.
    staged_path: Option<String>,
}

impl ExportAttachment {
    /// Creates attachment metadata for an export document without a staged copy.
    ///
    /// Inputs: Values copied from a persisted `Attachment`.
    /// Outputs: Renderer-ready attachment metadata.
    /// Side effects: None.
    pub fn new(
        attachment_id: Uuid,
        file_path: impl Into<String>,
        mime_type: impl Into<String>,
        original_timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            attachment_id,
            file_path: file_path.into(),
            mime_type: mime_type.into(),
            original_timestamp,
            staged_path: None,
        }
    }

    /// Records the export-owned copy that renderers may read or link to.
    ///
    /// Inputs: `staged_path` is a readable copy created by the export orchestrator.
    /// Outputs: The updated attachment metadata.
    /// Side effects: None.
    pub fn with_staged_path(mut self, staged_path: impl Into<String>) -> Self {
        self.staged_path = Some(staged_path.into());
        self
    }

    /// Returns the attachment's stable UUID.
    pub fn attachment_id(&self) -> Uuid {
        self.attachment_id
    }

    /// Borrows the original source file path.
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Borrows the declared MIME type.
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// Returns the original UTC acquisition timestamp.
    pub fn original_timestamp(&self) -> DateTime<Utc> {
        self.original_timestamp
    }

    /// Borrows the staged export copy, when one exists.
    pub fn staged_path(&self) -> Option<&str> {
        self.staged_path.as_deref()
    }

    /// Returns the source file name, falling back to `attachment` for path-less values.
    pub fn display_name(&self) -> &str {
        std::path::Path::new(&self.file_path)
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or("attachment")
    }

    /// Reports whether the declared MIME type is an image that renderers may show inline.
    pub fn is_image(&self) -> bool {
        self.mime_type.starts_with("image/")
    }
}

/// Represents one speaker-attributed transcript interval prepared for export rendering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportTranscriptSegment {
    start_time_ms: u64,
    end_time_ms: u64,
    /// The resolved identity, or `None` when the segment is unattributed.
    speaker_identity_id: Option<Uuid>,
    speaker: String,
    text: String,
    /// How doubtful the segment is (#90, #91).
    confidence: crate::domain::transcript_confidence::SegmentConfidence,
}

impl ExportTranscriptSegment {
    /// Attaches the segment's confidence marks. Side effects: None.
    pub fn with_confidence(
        mut self,
        confidence: crate::domain::transcript_confidence::SegmentConfidence,
    ) -> Self {
        self.confidence = confidence;
        self
    }

    /// Borrows the segment's confidence marks.
    pub fn confidence(&self) -> &crate::domain::transcript_confidence::SegmentConfidence {
        &self.confidence
    }

    /// Returns the speaker label, followed by ` (?)` when the speaker is a guess.
    pub fn marked_speaker(&self) -> String {
        if self.confidence.uncertain_speaker {
            format!("{} (?)", self.speaker)
        } else {
            self.speaker.clone()
        }
    }

    /// Splits the text into words, each with whether it was unclear.
    ///
    /// Outputs: The whitespace-separated words in order. Side effects: None.
    pub fn words_with_doubt(&self) -> Vec<(&str, bool)> {
        self.text
            .split_whitespace()
            .enumerate()
            .map(|(position, word)| {
                let uncertain = u16::try_from(position)
                    .is_ok_and(|position| self.confidence.uncertain_words.contains(&position));
                (word, uncertain)
            })
            .collect()
    }
    /// Creates one time-bounded transcript segment with a resolved speaker label.
    ///
    /// Inputs: Offsets from the meeting start in milliseconds, the optional identity UUID, the
    /// human-readable speaker label, and the segment text.
    /// Outputs: Renderer-ready segment.
    /// Side effects: None.
    pub fn new(
        start_time_ms: u64,
        end_time_ms: u64,
        speaker_identity_id: Option<Uuid>,
        speaker: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            start_time_ms,
            end_time_ms,
            speaker_identity_id,
            speaker: speaker.into(),
            text: text.into(),
            confidence: Default::default(),
        }
    }

    /// Returns the start offset in milliseconds.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the end offset in milliseconds.
    pub fn end_time_ms(&self) -> u64 {
        self.end_time_ms
    }

    /// Returns the attributed identity, if any.
    pub fn speaker_identity_id(&self) -> Option<Uuid> {
        self.speaker_identity_id
    }

    /// Borrows the human-readable speaker label.
    pub fn speaker(&self) -> &str {
        &self.speaker
    }

    /// Borrows the segment text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Carries all format-neutral data for one meeting that a renderer needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportDocument {
    meeting_id: MeetingId,
    /// The stored title, or the meeting's dated fallback title.
    title: String,
    /// Whether `title` was chosen by a person rather than generated as a fallback.
    has_stored_title: bool,
    started_at: DateTime<Utc>,
    transcript_segments: Vec<ExportTranscriptSegment>,
    attachments: Vec<ExportAttachment>,
    /// Identities referenced by at least one segment; never contains embeddings.
    voice_identities: Vec<SpeakerIdentity>,
}

impl ExportDocument {
    /// Creates a complete, immutable renderer input assembled by the application layer.
    ///
    /// Inputs: Meeting identity and title, UTC start instant, ordered segments, attachments, and
    /// the identities referenced by those segments.
    /// Outputs: The export document.
    /// Side effects: None.
    pub fn new(
        meeting_id: MeetingId,
        title: impl Into<String>,
        started_at: DateTime<Utc>,
        transcript_segments: Vec<ExportTranscriptSegment>,
        attachments: Vec<ExportAttachment>,
        voice_identities: Vec<SpeakerIdentity>,
    ) -> Self {
        Self {
            meeting_id,
            title: title.into(),
            has_stored_title: false,
            started_at,
            transcript_segments,
            attachments,
            voice_identities,
        }
    }

    /// Marks the title as person-chosen (`true`) or a generated fallback (`false`, the default).
    ///
    /// Inputs: Whether the meeting had a stored title.
    /// Outputs: The document.
    /// Side effects: None.
    pub fn with_stored_title(mut self, has_stored_title: bool) -> Self {
        self.has_stored_title = has_stored_title;
        self
    }

    /// Returns whether the title was chosen by a person rather than generated as a fallback.
    pub fn has_stored_title(&self) -> bool {
        self.has_stored_title
    }

    /// Replaces attachment metadata, typically after the orchestrator staged copies.
    pub fn with_attachments(mut self, attachments: Vec<ExportAttachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// Borrows the stable meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Borrows the display title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the UTC meeting start time.
    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// Borrows ordered transcript segments.
    pub fn transcript_segments(&self) -> &[ExportTranscriptSegment] {
        &self.transcript_segments
    }

    /// Borrows attachment metadata.
    pub fn attachments(&self) -> &[ExportAttachment] {
        &self.attachments
    }

    /// Borrows identities referenced by the transcript.
    pub fn voice_identities(&self) -> &[SpeakerIdentity] {
        &self.voice_identities
    }

    /// Returns the meeting duration as the latest segment end offset (0 without segments).
    pub fn duration_ms(&self) -> u64 {
        self.transcript_segments
            .iter()
            .map(ExportTranscriptSegment::end_time_ms)
            .max()
            .unwrap_or_default()
    }

    /// Returns distinct speaker labels in order of first appearance.
    pub fn participants(&self) -> Vec<&str> {
        let mut participants: Vec<&str> = Vec::new();
        for segment in &self.transcript_segments {
            if !participants.contains(&segment.speaker()) {
                participants.push(segment.speaker());
            }
        }
        participants
    }

    /// Counts whitespace-delimited words across all segments.
    pub fn word_count(&self) -> usize {
        self.transcript_segments
            .iter()
            .map(|segment| segment.text().split_whitespace().count())
            .sum()
    }

    /// Totals speaking time per speaker label, sorted by label for deterministic output.
    ///
    /// Inputs: None.
    /// Outputs: `(speaker, milliseconds)` pairs; inverted segment bounds count as zero.
    /// Side effects: None.
    pub fn speaking_time_by_speaker(&self) -> Vec<(String, u64)> {
        let mut speaking_totals = std::collections::BTreeMap::<&str, u64>::new();
        for segment in &self.transcript_segments {
            *speaking_totals.entry(segment.speaker()).or_default() += segment
                .end_time_ms()
                .saturating_sub(segment.start_time_ms());
        }
        speaking_totals
            .into_iter()
            .map(|(speaker, total_ms)| (String::from(speaker), total_ms))
            .collect()
    }
}

/// Formats a millisecond offset as zero-padded `HH:MM:SS` (hours grow beyond two digits if needed).
///
/// Inputs: `offset_ms` is a non-negative duration or offset.
/// Outputs: Text such as `01:02:05`; sub-second remainders are truncated.
/// Side effects: None.
pub fn format_export_offset(offset_ms: u64) -> String {
    let total_seconds = offset_ms / 1_000;
    format!(
        "{:02}:{:02}:{:02}",
        total_seconds / 3_600,
        (total_seconds % 3_600) / 60,
        total_seconds % 60
    )
}

/// Describes the lifecycle of an audio-capture session without exposing a platform stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioState {
    /// The capture session has not started and accepts a start command.
    Idle,
    /// The capture session routes audio chunks to subscribers.
    Recording,
    /// The capture session is retained but temporarily discards captured chunks.
    Paused,
    /// The OS suspended the active stream; a later start command may safely recover it.
    Interrupted,
    /// The capture session is closed and cannot be restarted.
    Stopped,
    /// The capture session encountered an unrecoverable adapter failure.
    Error,
}

/// Defines the portable PCM format requested from an audio-capture adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioConfig {
    sample_rate_hz: u32,
    channels: u16,
}

impl AudioConfig {
    /// Creates a PCM capture configuration.
    ///
    /// Inputs: `sample_rate_hz` and `channels` describe the requested stream format; validation occurs
    /// when an adapter opens a stream.
    /// Outputs: A portable configuration that contains no operating-system device details.
    /// Side effects: None.
    pub fn new(sample_rate_hz: u32, channels: u16) -> Self {
        Self {
            sample_rate_hz,
            channels,
        }
    }

    /// Returns the requested number of PCM samples captured per second.
    ///
    /// Inputs: None.
    /// Outputs: The requested sample rate in hertz.
    /// Side effects: None.
    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the requested number of interleaved PCM channels.
    ///
    /// Inputs: None.
    /// Outputs: The requested channel count.
    /// Side effects: None.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// Carries one adapter-produced signed 16-bit PCM chunk and its format metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioChunk {
    samples: Vec<i16>,
    config: AudioConfig,
}

/// Carries optional prior transcript text for one context-aware transcription request.
///
/// This portable value intentionally contains text only. Adapter-owned audio lookback handling and
/// chunk-boundary metadata remain separate concerns so the transcription port never exposes raw
/// audio outside the adapter boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TranscriptionContext {
    previous_text: Option<String>,
    language: Option<TranscriptionLanguage>,
    /// Names and terms to spell as the owner wrote them; never empty when set.
    glossary: Option<crate::domain::transcription_glossary::TranscriptionGlossary>,
}

impl TranscriptionContext {
    /// Creates context for one transcription request.
    ///
    /// Inputs: `previous_text` is the immediately preceding successful chunk transcript for the
    /// same meeting, when available. It must not contain raw audio or provider-specific values.
    /// Outputs: An immutable portable context for a transcription adapter.
    /// Side effects: Takes ownership of the optional transcript text.
    pub fn new(previous_text: Option<String>) -> Self {
        Self {
            previous_text,
            language: None,
            glossary: None,
        }
    }

    /// Sets the glossary this chunk is transcribed with.
    ///
    /// Inputs: The owner's glossary; `None` or an empty one means none.
    /// Outputs: The context. Side effects: None.
    pub fn with_glossary(
        mut self,
        glossary: Option<crate::domain::transcription_glossary::TranscriptionGlossary>,
    ) -> Self {
        self.glossary = glossary.filter(|glossary| !glossary.is_empty());
        self
    }

    /// Borrows the glossary, if any. Side effects: None.
    pub fn glossary(
        &self,
    ) -> Option<&crate::domain::transcription_glossary::TranscriptionGlossary> {
        self.glossary.as_ref()
    }

    /// Returns the text an engine should be prompted with: any earlier text, then the glossary's
    /// list, or `None` when there is neither. Whisper keeps only the end of a long prompt, so the
    /// glossary goes last, where it is kept and read as the most recent text.
    ///
    /// Side effects: None.
    pub fn prompt(&self) -> Option<String> {
        let parts: Vec<String> = self
            .previous_text
            .clone()
            .into_iter()
            .chain(
                self.glossary
                    .as_ref()
                    .and_then(crate::domain::transcription_glossary::TranscriptionGlossary::prompt),
            )
            .collect();
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    /// Sets the language this chunk is transcribed in.
    ///
    /// Inputs: The language chosen for the chunk; `None` leaves it to the engine's configuration.
    /// Outputs: The context. Side effects: None.
    pub fn with_language(mut self, language: Option<TranscriptionLanguage>) -> Self {
        self.language = language;
        self
    }

    /// Returns the language chosen for this chunk, if any. Side effects: None.
    pub fn language(&self) -> Option<TranscriptionLanguage> {
        self.language
    }

    /// Picks the language an engine should use.
    ///
    /// Inputs: The engine's configured language, which a choice for this chunk overrides.
    /// Outputs: The language's text (`auto` included), or `None` when neither is set.
    /// Side effects: None.
    pub fn language_or<'text>(&self, configured: Option<&'text str>) -> Option<&'text str> {
        match self.language {
            Some(language) => Some(language.as_str()),
            None => configured,
        }
    }

    /// Borrows the previous chunk transcript when context is available.
    ///
    /// Inputs: None.
    /// Outputs: The immediately preceding transcript text, or `None` for a meeting's first chunk.
    /// Side effects: None.
    pub fn previous_text(&self) -> Option<&str> {
        self.previous_text.as_deref()
    }
}

impl AudioChunk {
    /// Creates a portable signed 16-bit PCM chunk.
    ///
    /// Inputs: `samples` contains interleaved PCM values matching `config`.
    /// Outputs: An immutable chunk suitable for local processing or future P2P streaming adapters.
    /// Side effects: Takes ownership of the raw samples; callers must not log the returned value.
    pub fn new(samples: Vec<i16>, config: AudioConfig) -> Self {
        Self { samples, config }
    }

    /// Borrows the PCM samples without copying the raw audio content.
    ///
    /// Inputs: None.
    /// Outputs: The interleaved signed 16-bit PCM samples.
    /// Side effects: None.
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    /// Returns the format metadata associated with this audio chunk.
    ///
    /// Inputs: None.
    /// Outputs: The chunk's portable PCM configuration.
    /// Side effects: None.
    pub fn config(&self) -> AudioConfig {
        self.config
    }

    /// Takes ownership of the PCM samples without copying them.
    ///
    /// Inputs: None.
    /// Outputs: The interleaved signed 16-bit PCM samples.
    /// Side effects: Consumes the chunk.
    pub fn into_samples(self) -> Vec<i16> {
        self.samples
    }
}

/// Identifies a meeting across offline replicas.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MeetingId(String);

impl MeetingId {
    /// Creates a meeting identifier from an application-generated stable value.
    ///
    /// Inputs: `value` must be unique within the chosen identifier scheme.
    /// Outputs: A typed meeting identifier.
    /// Side effects: None.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the identifier text for adapter boundary metadata.
    ///
    /// Inputs: None.
    /// Outputs: The meeting identifier text.
    /// Side effects: None.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Represents the recorded time range used to associate externally acquired attachments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingWindow {
    meeting_id: MeetingId,
    meeting_start_time: DateTime<Utc>,
    meeting_end_time: DateTime<Utc>,
}

impl MeetingWindow {
    /// Creates a meeting window from recording lifecycle timestamps.
    ///
    /// Inputs: `meeting_id` identifies the meeting; timestamps must be UTC and end no earlier than start.
    /// Outputs: An immutable time window for repository matching.
    /// Side effects: None.
    pub fn new(
        meeting_id: MeetingId,
        meeting_start_time: DateTime<Utc>,
        meeting_end_time: DateTime<Utc>,
    ) -> Self {
        Self {
            meeting_id,
            meeting_start_time,
            meeting_end_time,
        }
    }

    /// Returns the meeting identifier owning this time window.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the stable meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the inclusive UTC start of this meeting window.
    ///
    /// Inputs: None.
    /// Outputs: The recorded meeting start timestamp.
    /// Side effects: None.
    pub fn meeting_start_time(&self) -> DateTime<Utc> {
        self.meeting_start_time
    }

    /// Returns the inclusive UTC end of this meeting window.
    ///
    /// Inputs: None.
    /// Outputs: The recorded meeting end timestamp.
    /// Side effects: None.
    pub fn meeting_end_time(&self) -> DateTime<Utc> {
        self.meeting_end_time
    }

    /// Reports whether a timestamp lies within this inclusive meeting time range.
    ///
    /// Inputs: `timestamp` is an attachment capture timestamp in UTC.
    /// Outputs: `true` when the timestamp is between the recorded start and end, inclusive.
    /// Side effects: None.
    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        self.meeting_start_time <= timestamp && timestamp <= self.meeting_end_time
    }
}

/// Describes a user-added file and its safe-to-persist acquisition metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    attachment_id: Uuid,
    meeting_id: Option<MeetingId>,
    file_path: String,
    mime_type: String,
    original_timestamp: DateTime<Utc>,
    original_device_id: String,
    /// Meeting time (ms) the attachment belongs at in the transcript; `None` places it at the end.
    meeting_time_ms: Option<u64>,
    /// The project that owns this attachment as one of its resources (Issue #85); such an
    /// attachment has no meeting.
    project_id: Option<crate::domain::projects::ProjectId>,
}

/// What an attachment belongs to: a meeting, or a project as one of its resources (Issue #85).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AttachmentOwner {
    /// An attachment of a meeting.
    Meeting(MeetingId),
    /// A resource of a project.
    Project(crate::domain::projects::ProjectId),
}

/// One linked attachment as paired devices describe it: metadata only, never a local path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedAttachment {
    /// The ID shared by every copy of this attachment.
    pub attachment_id: Uuid,
    /// The meeting or project it belongs to.
    pub owner: AttachmentOwner,
    /// The file's name, without any folder.
    pub file_name: String,
    /// The file's MIME type.
    pub mime_type: String,
    /// The lowercase hexadecimal SHA-256 of the file.
    pub content_hash: String,
    /// The file's length in bytes.
    pub size_bytes: u64,
    /// When the attachment was added.
    pub original_timestamp: DateTime<Utc>,
    /// The device that added it.
    pub original_device_id: String,
    /// Where it sits in the meeting's transcript, if placed.
    pub meeting_time_ms: Option<u64>,
}

/// One bounded piece of an attachment file on its way to a paired device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChunk {
    hash: String,
    total_size: u64,
    offset: u64,
    data: Vec<u8>,
}

impl FileChunk {
    /// Creates a chunk; the receiver validates every field.
    pub fn new(hash: impl Into<String>, total_size: u64, offset: u64, data: Vec<u8>) -> Self {
        Self {
            hash: hash.into(),
            total_size,
            offset,
            data,
        }
    }
    /// Borrows the file's content address.
    pub fn hash(&self) -> &str {
        &self.hash
    }
    /// Returns the complete file's size.
    pub fn total_size(&self) -> u64 {
        self.total_size
    }
    /// Returns this chunk's zero-based byte offset.
    pub fn offset(&self) -> u64 {
        self.offset
    }
    /// Borrows the file bytes in this chunk.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Attachment {
    /// Creates an attachment record without reading the file or assigning it to a meeting.
    ///
    /// Inputs: `attachment_id` must be unique; `meeting_id` remains absent until user confirmation.
    /// Outputs: An immutable attachment suitable for an `AttachmentRepository`.
    /// Side effects: Takes ownership of metadata only; it does not access or log the file.
    pub fn new(
        attachment_id: Uuid,
        meeting_id: Option<MeetingId>,
        file_path: impl Into<String>,
        mime_type: impl Into<String>,
        original_timestamp: DateTime<Utc>,
        original_device_id: impl Into<String>,
    ) -> Self {
        Self {
            attachment_id,
            meeting_id,
            file_path: file_path.into(),
            mime_type: mime_type.into(),
            original_timestamp,
            original_device_id: original_device_id.into(),
            meeting_time_ms: None,
            project_id: None,
        }
    }

    /// Makes the attachment a resource of `project_id` (Issue #85).
    ///
    /// Inputs: The owning project; the attachment should have no meeting.
    /// Outputs: The attachment. Side effects: None.
    pub fn with_project_id(mut self, project_id: crate::domain::projects::ProjectId) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Returns the project that owns this attachment as a resource, if any (Issue #85).
    pub fn project_id(&self) -> Option<crate::domain::projects::ProjectId> {
        self.project_id
    }

    /// Places the attachment at a point of its meeting's transcript (Issue #54).
    ///
    /// Inputs: The meeting time in milliseconds, e.g. when a photo was taken during a recording.
    /// Outputs: The attachment. Side effects: None.
    pub fn with_meeting_time_ms(mut self, meeting_time_ms: u64) -> Self {
        self.meeting_time_ms = Some(meeting_time_ms);
        self
    }

    /// Returns the meeting time the attachment belongs at, or `None` for the end of the meeting.
    /// Inputs: None.
    /// Outputs: Milliseconds of meeting time, when known.
    /// Side effects: None.
    pub fn meeting_time_ms(&self) -> Option<u64> {
        self.meeting_time_ms
    }

    /// Returns the attachment's globally unique identifier.
    /// Inputs: None.
    /// Outputs: The attachment UUID.
    /// Side effects: None.
    pub fn attachment_id(&self) -> Uuid {
        self.attachment_id
    }

    /// Returns the assigned meeting, if the attachment has been confirmed or linked.
    /// Inputs: None.
    /// Outputs: An optional borrow of the owning meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> Option<&MeetingId> {
        self.meeting_id.as_ref()
    }

    /// Borrows the local path used by an adapter to locate this attachment.
    /// Inputs: None.
    /// Outputs: The caller-supplied local file path.
    /// Side effects: None.
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Borrows the attachment MIME type.
    /// Inputs: None.
    /// Outputs: The declared MIME type.
    /// Side effects: None.
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// The name the file is shown with: its stored file name without the `<attachment ID>-`
    /// prefix that keeps stored names unique.
    /// Inputs: None. Outputs: The name, empty when the path has none. Side effects: None.
    pub fn shown_file_name(&self) -> String {
        let stored_name = std::path::Path::new(&self.file_path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        stored_name
            .strip_prefix(&format!("{}-", self.attachment_id))
            .map(str::to_owned)
            .unwrap_or(stored_name)
    }

    /// Returns the original UTC timestamp obtained through a metadata adapter.
    /// Inputs: None.
    /// Outputs: The attachment acquisition timestamp.
    /// Side effects: None.
    pub fn original_timestamp(&self) -> DateTime<Utc> {
        self.original_timestamp
    }

    /// Borrows the source device identifier recorded with this attachment.
    /// Inputs: None.
    /// Outputs: The source device identifier.
    /// Side effects: None.
    pub fn original_device_id(&self) -> &str {
        &self.original_device_id
    }
}

/// Ranks one meeting as a possible owner of an attachment.
#[derive(Clone, Debug, PartialEq)]
pub struct MeetingSuggestion {
    meeting_id: MeetingId,
    meeting_start_time: DateTime<Utc>,
    meeting_end_time: DateTime<Utc>,
    confidence_score: f32,
}

impl MeetingSuggestion {
    /// Creates a meeting suggestion with a normalized confidence score.
    ///
    /// Inputs: timestamps identify the matching window; score must be between zero and one.
    /// Outputs: An immutable suggestion for presentation or user confirmation.
    /// Side effects: None.
    pub fn new(
        meeting_id: MeetingId,
        meeting_start_time: DateTime<Utc>,
        meeting_end_time: DateTime<Utc>,
        confidence_score: f32,
    ) -> Self {
        Self {
            meeting_id,
            meeting_start_time,
            meeting_end_time,
            confidence_score,
        }
    }

    /// Returns the candidate meeting identifier.
    /// Inputs: None.
    /// Outputs: A borrow of the candidate meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the candidate meeting start timestamp.
    /// Inputs: None.
    /// Outputs: The UTC meeting start timestamp.
    /// Side effects: None.
    pub fn meeting_start_time(&self) -> DateTime<Utc> {
        self.meeting_start_time
    }

    /// Returns the candidate meeting end timestamp.
    /// Inputs: None.
    /// Outputs: The UTC meeting end timestamp.
    /// Side effects: None.
    pub fn meeting_end_time(&self) -> DateTime<Utc> {
        self.meeting_end_time
    }

    /// Returns the normalized time-window confidence score.
    /// Inputs: None.
    /// Outputs: A score from zero at an edge to one at the midpoint.
    /// Side effects: None.
    pub fn confidence_score(&self) -> f32 {
        self.confidence_score
    }
}

/// Identifies an audio recording belonging to a meeting.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RecordingId(String);

impl RecordingId {
    /// Creates a recording identifier from an application-generated stable value.
    ///
    /// Inputs: `value` must be unique within the chosen identifier scheme.
    /// Outputs: A typed recording identifier.
    /// Side effects: None.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the identifier value, e.g. as an audio-store key.
    ///
    /// Inputs: None.
    /// Outputs: The identifier text.
    /// Side effects: None.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identifies the node that acquired data or emitted an operational event.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    /// Creates a device identifier from the node's stable local identity.
    ///
    /// Inputs: `value` should be a non-secret, stable node identifier.
    /// Outputs: A typed device identifier.
    /// Side effects: None.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the stable identifier value for metadata fields.
    ///
    /// Inputs: None.
    /// Outputs: The identifier text.
    /// Side effects: None.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Carries one serialized Automerge sync-protocol message for a specific meeting document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncMessage {
    meeting_id: MeetingId,
    automerge_sync_bytes: Vec<u8>,
}

impl SyncMessage {
    /// Creates a sync envelope for one meeting document.
    ///
    /// Inputs: `meeting_id` identifies the target document and `automerge_sync_bytes` contains an
    /// Automerge sync message encoded by the application CRDT service.
    /// Outputs: A transport-neutral envelope ready for encrypted P2P transmission.
    /// Side effects: Takes ownership of protocol bytes without inspecting or logging note content.
    pub fn new(meeting_id: MeetingId, automerge_sync_bytes: Vec<u8>) -> Self {
        Self {
            meeting_id,
            automerge_sync_bytes,
        }
    }

    /// Returns the meeting document targeted by this protocol message.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the target meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the opaque Automerge sync-protocol bytes without copying them.
    ///
    /// Inputs: None.
    /// Outputs: The encoded Automerge sync message.
    /// Side effects: None.
    pub fn sync_bytes(&self) -> &[u8] {
        &self.automerge_sync_bytes
    }

    /// Serializes this envelope with a versioned, length-delimited binary framing.
    ///
    /// Inputs: None.
    /// Outputs: Bytes suitable only for the encrypted P2P transport, or a framing error.
    /// Side effects: Allocates an output buffer; it does not log or persist sync content.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SyncMessageEncodeError> {
        const SYNC_ENVELOPE_HEADER: [u8; 4] = *b"NTS1";

        let meeting_id_bytes = self.meeting_id.as_str().as_bytes();
        let meeting_id_length = u32::try_from(meeting_id_bytes.len()).map_err(|_| {
            SyncMessageEncodeError::MeetingIdentifierTooLong {
                byte_length: meeting_id_bytes.len(),
            }
        })?;
        let mut serialized_message = Vec::with_capacity(
            SYNC_ENVELOPE_HEADER
                .len()
                .saturating_add(std::mem::size_of::<u32>())
                .saturating_add(meeting_id_bytes.len())
                .saturating_add(self.automerge_sync_bytes.len()),
        );
        serialized_message.extend_from_slice(&SYNC_ENVELOPE_HEADER);
        serialized_message.extend_from_slice(&meeting_id_length.to_be_bytes());
        serialized_message.extend_from_slice(meeting_id_bytes);
        serialized_message.extend_from_slice(&self.automerge_sync_bytes);
        Ok(serialized_message)
    }

    /// Deserializes a versioned sync envelope received through encrypted transport.
    ///
    /// Inputs: `serialized_message` must contain the framing produced by `to_bytes`.
    /// Outputs: A typed envelope or a validation error for malformed peer-controlled bytes.
    /// Side effects: Allocates only decoded metadata and payload bytes; it does not log content.
    pub fn from_bytes(serialized_message: &[u8]) -> Result<Self, SyncMessageDecodeError> {
        const SYNC_ENVELOPE_HEADER: [u8; 4] = *b"NTS1";
        let header_and_length_size = SYNC_ENVELOPE_HEADER
            .len()
            .saturating_add(std::mem::size_of::<u32>());
        if serialized_message.len() < header_and_length_size {
            return Err(SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope is shorter than its header"),
            });
        }
        if serialized_message[..SYNC_ENVELOPE_HEADER.len()] != SYNC_ENVELOPE_HEADER {
            return Err(SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope has an unknown protocol header"),
            });
        }

        let meeting_id_length_bytes: [u8; 4] = serialized_message
            [SYNC_ENVELOPE_HEADER.len()..header_and_length_size]
            .try_into()
            .map_err(|_| SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope contains an invalid meeting identifier length"),
            })?;
        let meeting_id_length = usize::try_from(u32::from_be_bytes(meeting_id_length_bytes))
            .map_err(|_| SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope meeting identifier length is unsupported"),
            })?;
        let meeting_id_end = header_and_length_size
            .checked_add(meeting_id_length)
            .ok_or_else(|| SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope meeting identifier length overflows"),
            })?;
        if serialized_message.len() < meeting_id_end {
            return Err(SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope is truncated before its meeting identifier"),
            });
        }
        let meeting_id =
            std::str::from_utf8(&serialized_message[header_and_length_size..meeting_id_end])
                .map_err(|_| SyncMessageDecodeError::InvalidEnvelope {
                    reason: String::from("sync envelope meeting identifier is not UTF-8"),
                })?;

        Ok(Self::new(
            MeetingId::new(meeting_id),
            serialized_message[meeting_id_end..].to_vec(),
        ))
    }
}

/// Describes malformed peer-controlled sync-envelope bytes.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SyncMessageDecodeError {
    /// The envelope does not satisfy the versioned binary framing contract.
    #[error("invalid sync message envelope: {reason}")]
    InvalidEnvelope {
        /// Explains the failed validation without including sync payload bytes.
        reason: String,
    },
}

/// Describes a local sync-envelope serialization failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SyncMessageEncodeError {
    /// The meeting identifier cannot be represented by the protocol's 32-bit length field.
    #[error("meeting identifier is too long for a sync message: {byte_length} bytes")]
    MeetingIdentifierTooLong {
        /// The unrepresentable UTF-8 identifier length.
        byte_length: usize,
    },
}

/// Identifies a transcription model that a device can execute without exposing its implementation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TranscriptionModel {
    /// The node can execute a Whisper-compatible transcription workload.
    Whisper,
    /// The node can execute a named provider or future model integration.
    ThirdParty(String),
}

impl TranscriptionModel {
    /// Returns a stable non-sensitive model label for telemetry persistence.
    ///
    /// Inputs: None.
    /// Outputs: A borrowed label; provider names retain their configured text.
    /// Side effects: None.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Whisper => "whisper",
            Self::ThirdParty(model_name) => model_name,
        }
    }
}

/// Advertises a node's non-sensitive AI capacity for decentralized workload selection.
///
/// Paired devices exchange it over the encrypted P2P transport
/// (`application::device_manifest_exchange`). It intentionally contains no
/// credentials, model weights, raw audio, or hardware serial numbers.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeviceManifest {
    device_id: DeviceId,
    cpu_cores: u16,
    gpu_vram_megabytes: u32,
    available_transcription_models: Vec<TranscriptionModel>,
    /// Reports whether this device can run feature-gated local VAD chunking.
    pub supports_vad: bool,
    /// Which local Whisper model runs here, such as `ggml-small`; absent from older devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_whisper_model: Option<String>,
    /// The ID this device writes on the meetings it records, which differs from its peer ID;
    /// paired devices use it to tell which device recorded a meeting (#88). Absent from older
    /// devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_device_id: Option<DeviceId>,
}

impl DeviceManifest {
    /// Creates a portable capability manifest for one decentralized node.
    ///
    /// Inputs: `device_id` identifies the node; CPU and GPU values describe available capacity;
    /// `available_transcription_models` lists workloads that the node can execute.
    /// Outputs: An immutable manifest suitable for encrypted P2P broadcast and routing.
    /// Side effects: Takes ownership of model capability metadata only; it performs no hardware scan.
    pub fn new(
        device_id: DeviceId,
        cpu_cores: u16,
        gpu_vram_megabytes: u32,
        available_transcription_models: Vec<TranscriptionModel>,
    ) -> Self {
        Self {
            device_id,
            cpu_cores,
            gpu_vram_megabytes,
            available_transcription_models,
            supports_vad: cfg!(feature = "vad"),
            local_whisper_model: None,
            storage_device_id: None,
        }
    }

    /// Returns the stable identifier of the device that advertised this manifest.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the originating device identifier.
    /// Side effects: None.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// Returns the advertised number of CPU cores.
    ///
    /// Inputs: None. Outputs: The core count. Side effects: None.
    pub fn cpu_cores(&self) -> u16 {
        self.cpu_cores
    }

    /// Returns the transcription models this node can run.
    ///
    /// Inputs: None. Outputs: The advertised models, in the order given. Side effects: None.
    pub fn available_transcription_models(&self) -> &[TranscriptionModel] {
        &self.available_transcription_models
    }

    /// Returns advertised GPU VRAM for telemetry's coarse CPU/GPU power heuristic.
    ///
    /// Inputs: None.
    /// Outputs: Megabytes of GPU VRAM advertised by the device manifest.
    /// Side effects: None.
    pub fn gpu_vram_megabytes(&self) -> u32 {
        self.gpu_vram_megabytes
    }

    /// Reports whether this node advertises support for a requested transcription model.
    ///
    /// Inputs: `transcription_model` is the model required by a workload.
    /// Outputs: `true` only when the model is present in this manifest.
    /// Side effects: None.
    pub fn supports_transcription_model(&self, transcription_model: &TranscriptionModel) -> bool {
        self.available_transcription_models
            .iter()
            .any(|available_model| available_model == transcription_model)
    }

    /// Produces a deterministic capacity score used only by the automatic routing policy.
    ///
    /// Inputs: None.
    /// Outputs: A score prioritizing GPU memory, then CPU cores, without identifying hardware.
    /// Side effects: None.
    pub fn transcription_capacity_score(&self) -> u64 {
        u64::from(self.gpu_vram_megabytes) * 1_000 + u64::from(self.cpu_cores)
    }
}

#[cfg(test)]
mod device_manifest_tests {
    //! Defines feature-sensitive serialization contracts for the device VAD capability flag.

    use super::{DeviceId, DeviceManifest, TranscriptionModel};

    fn manifest() -> DeviceManifest {
        DeviceManifest::new(
            DeviceId::new("vad-capability-test-device"),
            4,
            0,
            vec![TranscriptionModel::Whisper],
        )
    }

    #[test]
    #[cfg(feature = "vad")]
    fn smart_node_manifest_has_supports_vad_when_feature_enabled() {
        assert!(manifest().supports_vad);
    }

    #[test]
    #[cfg(not(feature = "vad"))]
    fn dumb_node_manifest_has_supports_vad_when_feature_disabled() {
        assert!(!manifest().supports_vad);
    }

    #[test]
    fn manifest_serialization_includes_supports_vad_field() {
        let serialized = serde_json::to_value(manifest())
            .unwrap_or_else(|error| panic!("manifest should serialize: {error}"));
        assert_eq!(
            serialized
                .get("supports_vad")
                .and_then(|value| value.as_bool()),
            Some(cfg!(feature = "vad"))
        );
        let round_tripped: DeviceManifest = serde_json::from_value(serialized)
            .unwrap_or_else(|error| panic!("manifest should deserialize: {error}"));
        assert_eq!(round_tripped.supports_vad, cfg!(feature = "vad"));
    }
}

/// Represents a captured recording without retaining raw audio content in the domain model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioRecording {
    recording_id: RecordingId,
    meeting_id: MeetingId,
    acquisition_device_id: DeviceId,
}

impl AudioRecording {
    /// Builds recording metadata for work passed to a transcription port.
    ///
    /// Inputs: Identifiers must refer to the captured recording, owning meeting, and acquiring node.
    /// Outputs: Immutable recording metadata; raw audio remains adapter-owned.
    /// Side effects: None.
    pub fn new(
        recording_id: RecordingId,
        meeting_id: MeetingId,
        acquisition_device_id: DeviceId,
    ) -> Self {
        Self {
            recording_id,
            meeting_id,
            acquisition_device_id,
        }
    }

    /// Returns the owning meeting identifier.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the recording (or chunk) identifier adapters use to locate its audio.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the recording identifier.
    /// Side effects: None.
    pub fn recording_id(&self) -> &RecordingId {
        &self.recording_id
    }
}

/// Represents text produced by a transcription engine for a meeting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transcript {
    meeting_id: MeetingId,
    content: String,
    timed_words: Vec<TimedWord>,
}

/// One transcribed word with its time, relative to the start of the transcribed audio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimedWord {
    text: String,
    start_ms: u64,
    end_ms: u64,
    /// How sure the engine was of the word, in thousandths (its least sure token); `None` when
    /// the engine gave no probability (#91).
    probability_permille: Option<u16>,
}

impl TimedWord {
    /// Creates a timed word.
    ///
    /// Inputs: The word's text and its offsets in milliseconds; an end before the start is raised
    /// to the start.
    /// Outputs: An immutable word.
    /// Side effects: None.
    pub fn new(text: impl Into<String>, start_ms: u64, end_ms: u64) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms: end_ms.max(start_ms),
            probability_permille: None,
        }
    }

    /// Records how sure the engine was of the word.
    ///
    /// Inputs: A probability in [0, 1]; values outside are clamped, and NaN records none.
    /// Outputs: The same word with its probability in thousandths. Side effects: None.
    pub fn with_probability(self, probability: f32) -> Self {
        let permille =
            (!probability.is_nan()).then(|| (probability.clamp(0.0, 1.0) * 1000.0).round() as u16);
        self.with_probability_permille(permille)
    }

    /// Records the word's probability in thousandths, or none (values above 1000 are clamped).
    /// Side effects: None.
    pub fn with_probability_permille(mut self, probability_permille: Option<u16>) -> Self {
        self.probability_permille = probability_permille.map(|permille| permille.min(1000));
        self
    }

    /// Returns how sure the engine was of the word, in thousandths, if it said.
    pub fn probability_permille(&self) -> Option<u16> {
        self.probability_permille
    }

    /// Borrows the word's text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns when the word starts, in chunk milliseconds.
    pub fn start_ms(&self) -> u64 {
        self.start_ms
    }

    /// Returns when the word ends, in chunk milliseconds.
    pub fn end_ms(&self) -> u64 {
        self.end_ms
    }

    /// Returns the moment that decides who said the word, in chunk milliseconds.
    pub fn midpoint_ms(&self) -> u64 {
        self.start_ms + (self.end_ms - self.start_ms) / 2
    }
}

/// Categorizes AI work for privacy-safe telemetry grouping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskType {
    /// Converts a meeting recording into transcript text.
    Transcription,
    /// Separates speakers and derives segment embeddings.
    Diarization,
    /// Transfers a verified content-addressed attachment over encrypted P2P transport.
    FileTransfer,
    /// Executes transcription through a cloud API provider.
    CloudTranscription,
}

impl TaskType {
    /// Returns the stable storage label for this task category.
    ///
    /// Inputs: None.
    /// Outputs: A lowercase identifier safe for telemetry grouping.
    /// Side effects: None.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transcription => "transcription",
            Self::Diarization => "diarization",
            Self::FileTransfer => "file_transfer",
            Self::CloudTranscription => "cloud_transcription",
        }
    }
}

/// Records one completed AI task without retaining inputs, outputs, or biometric data.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskMetrics {
    task_id: Uuid,
    device_id: DeviceId,
    task_type: TaskType,
    model_name: String,
    duration_ms: u64,
    latency_ms: u64,
    estimated_power_mwh: f32,
    timestamp_epoch_ms: u64,
    /// The length of the audio transcribed; `None` for other tasks and older records.
    audio_duration_ms: Option<u64>,
}

impl TaskMetrics {
    /// Creates one completed-task telemetry record.
    ///
    /// Inputs: IDs and model label identify the execution; timing is milliseconds; power is an
    /// estimated milli-watt-hour value; timestamp is UTC Unix epoch milliseconds.
    /// Outputs: An immutable metrics value suitable for `MetricsRepository` persistence.
    /// Side effects: Takes ownership of non-sensitive model metadata; never records task payloads.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: Uuid,
        device_id: DeviceId,
        task_type: TaskType,
        model_name: impl Into<String>,
        duration_ms: u64,
        latency_ms: u64,
        estimated_power_mwh: f32,
        timestamp_epoch_ms: u64,
    ) -> Self {
        Self {
            task_id,
            device_id,
            task_type,
            model_name: model_name.into(),
            duration_ms,
            latency_ms,
            estimated_power_mwh,
            timestamp_epoch_ms,
            audio_duration_ms: None,
        }
    }

    /// Records how much audio the task handled, which holds no audio or transcript.
    ///
    /// Inputs: The length, or `None` when unknown. Outputs: The record. Side effects: None.
    pub fn with_audio_duration_ms(mut self, audio_duration_ms: Option<u64>) -> Self {
        self.audio_duration_ms = audio_duration_ms;
        self
    }

    /// Returns how much audio the task handled, when recorded.
    pub fn audio_duration_ms(&self) -> Option<u64> {
        self.audio_duration_ms
    }

    /// Returns the unique task execution ID.
    pub fn task_id(&self) -> Uuid {
        self.task_id
    }
    /// Returns the device that executed the AI task.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
    /// Returns the completed AI task category.
    pub fn task_type(&self) -> TaskType {
        self.task_type
    }
    /// Returns the model label without exposing task data.
    pub fn model_name(&self) -> &str {
        &self.model_name
    }
    /// Returns elapsed engine execution time in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
    /// Returns observed end-to-end task latency in milliseconds.
    pub fn latency_ms(&self) -> u64 {
        self.latency_ms
    }
    /// Returns the heuristic energy estimate in milli-watt-hours.
    pub fn estimated_power_mwh(&self) -> f32 {
        self.estimated_power_mwh
    }
    /// Returns the UTC Unix epoch completion timestamp in milliseconds.
    pub fn timestamp_epoch_ms(&self) -> u64 {
        self.timestamp_epoch_ms
    }
}

/// Captures a device's non-sensitive instantaneous compute utilization.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceLoadStats {
    device_id: DeviceId,
    current_cpu_load_percent: f32,
    current_gpu_vram_used_megabytes: u32,
    active_tasks_count: u32,
}

impl DeviceLoadStats {
    /// Creates one device-load observation.
    ///
    /// Inputs: `current_cpu_load_percent` is normalized 0–100; VRAM and task count are snapshots.
    /// Outputs: An immutable load value for optional local telemetry persistence.
    /// Side effects: None.
    pub fn new(
        device_id: DeviceId,
        current_cpu_load_percent: f32,
        current_gpu_vram_used_megabytes: u32,
        active_tasks_count: u32,
    ) -> Self {
        Self {
            device_id,
            current_cpu_load_percent,
            current_gpu_vram_used_megabytes,
            active_tasks_count,
        }
    }
    /// Returns the observed device identifier.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
    /// Returns current CPU utilization as a percentage.
    pub fn current_cpu_load_percent(&self) -> f32 {
        self.current_cpu_load_percent
    }
    /// Returns current GPU VRAM use in megabytes.
    pub fn current_gpu_vram_used_megabytes(&self) -> u32 {
        self.current_gpu_vram_used_megabytes
    }
    /// Returns the number of active AI tasks.
    pub fn active_tasks_count(&self) -> u32 {
        self.active_tasks_count
    }
}

/// Summarizes persisted AI task metrics across devices and task categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregatedStats {
    task_count: u64,
    total_duration_ms: u64,
    average_latency_ms: u64,
}

impl AggregatedStats {
    /// Creates an aggregate summary returned by the telemetry repository.
    pub fn new(task_count: u64, total_duration_ms: u64, average_latency_ms: u64) -> Self {
        Self {
            task_count,
            total_duration_ms,
            average_latency_ms,
        }
    }
    /// Returns completed metric records included in the aggregate.
    pub fn task_count(&self) -> u64 {
        self.task_count
    }
    /// Returns cumulative AI processing time in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_ms
    }
    /// Returns integer-truncated average end-to-end latency in milliseconds.
    pub fn average_latency_ms(&self) -> u64 {
        self.average_latency_ms
    }
}

/// A speaker identity as paired devices exchange it (Issue #56).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SharedSpeakerIdentity {
    /// The person's stable UUID, shared across devices.
    pub identity_id: Uuid,
    /// The user-visible name; the newest rename wins.
    pub display_name: String,
    /// When the identity was created.
    pub created_at: DateTime<Utc>,
    /// When the name last changed.
    pub updated_at: DateTime<Utc>,
}

/// A voice variant as paired devices exchange it; the vector is biometric and never logged.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedVoiceVariant {
    /// The person this variant belongs to.
    pub identity_id: Uuid,
    /// The variant's stable UUID, shared across devices.
    pub variant_id: Uuid,
    /// The derived embedding, never raw audio.
    pub vector: Vec<f32>,
    /// When the variant was enrolled.
    pub created_at: DateTime<Utc>,
}

/// Records that an identity was deleted, so no paired device brings its voice back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityTombstone {
    /// The deleted person.
    pub identity_id: Uuid,
    /// When it was first deleted.
    pub deleted_at: DateTime<Utc>,
}

/// The voice library, or the part of it one message carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoiceLibrary {
    /// The people.
    pub identities: Vec<SharedSpeakerIdentity>,
    /// Their voice variants.
    pub variants: Vec<SharedVoiceVariant>,
    /// The people deleted.
    pub tombstones: Vec<IdentityTombstone>,
}

/// Identifies a person whose voice signatures may have multiple acoustic variants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerIdentity {
    identity_id: Uuid,
    display_name: String,
}

impl SpeakerIdentity {
    /// Creates a speaker identity with an application-generated UUID and user-visible name.
    ///
    /// Inputs: `identity_id` must uniquely identify the person across replicas; `display_name` is
    /// a user-provided label and must not be treated as an authentication credential.
    /// Outputs: An immutable speaker identity without biometric data.
    /// Side effects: Takes ownership of the display name; it does not persist or log it.
    pub fn new(identity_id: Uuid, display_name: impl Into<String>) -> Self {
        Self {
            identity_id,
            display_name: display_name.into(),
        }
    }

    /// Returns the stable UUID associated with this person.
    ///
    /// Inputs: None.
    /// Outputs: The identity UUID by value.
    /// Side effects: None.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// Borrows the user-visible speaker label.
    ///
    /// Inputs: None.
    /// Outputs: The display name without copying it.
    /// Side effects: None.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// Identifies a participant supplied by an external meeting integration without provider coupling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalParticipant {
    external_id: String,
    display_name: String,
    email: Option<String>,
}

impl ExternalParticipant {
    /// Creates externally supplied identity metadata for stable local mapping and UI naming.
    ///
    /// Inputs: `external_id` must be provider-stable; `display_name` is the initial UI label;
    /// `email` is optional contact metadata and must not be logged by default.
    /// Outputs: An immutable external participant value with no authentication credential.
    /// Side effects: Takes ownership of identifier and display metadata only.
    pub fn new(
        external_id: impl Into<String>,
        display_name: impl Into<String>,
        email: Option<String>,
    ) -> Self {
        Self {
            external_id: external_id.into(),
            display_name: display_name.into(),
            email,
        }
    }

    /// Borrows the provider-stable identifier used for durable external-to-local mappings.
    ///
    /// Inputs: None.
    /// Outputs: A non-secret stable external identifier.
    /// Side effects: None.
    pub fn external_id(&self) -> &str {
        &self.external_id
    }

    /// Borrows the user-visible participant name used when auto-creating a local identity.
    ///
    /// Inputs: None.
    /// Outputs: The integration-provided display name.
    /// Side effects: None.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Borrows optional contact metadata without treating it as an identity key.
    ///
    /// Inputs: None.
    /// Outputs: An optional integration-provided email address.
    /// Side effects: None.
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }
}

/// What an external audio track carries (Issue #102).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ExternalTrackKind {
    /// One participant's own audio: a bot's isolated track, or the owner's microphone in the
    /// browser.
    #[default]
    Participant,
    /// Every remote participant mixed into one track, as a browser receives a call. Its voices
    /// are told apart and matched against the gallery; it is never credited to one participant.
    MixedRemote,
}

/// The sample rate of the 16-bit mono PCM every online-meeting bridge delivers.
pub const EXTERNAL_AUDIO_SAMPLE_RATE_HZ: u32 = 16_000;

/// Carries an isolated external-speaker audio buffer with guaranteed participant metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMediaChunk {
    external_participant: ExternalParticipant,
    audio_bytes: Vec<u8>,
    timestamp_ms: u64,
    track_kind: ExternalTrackKind,
}

impl ExternalMediaChunk {
    /// Creates one opaque external audio chunk from a bridge subscription.
    ///
    /// Inputs: `external_participant` identifies the isolated speaker; `audio_bytes` must never be
    /// logged; `timestamp_ms` is relative to the external meeting stream.
    /// Outputs: An immutable bridge-neutral media value.
    /// Side effects: Takes ownership of raw audio bytes without persisting them.
    pub fn new(
        external_participant: ExternalParticipant,
        audio_bytes: Vec<u8>,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            external_participant,
            audio_bytes,
            timestamp_ms,
            track_kind: ExternalTrackKind::Participant,
        }
    }

    /// Marks what the track carries (a participant's track when never called).
    ///
    /// Inputs: The track kind. Outputs: The chunk. Side effects: None.
    pub fn with_track_kind(mut self, track_kind: ExternalTrackKind) -> Self {
        self.track_kind = track_kind;
        self
    }

    /// Returns what the track carries. Side effects: None.
    pub fn track_kind(&self) -> ExternalTrackKind {
        self.track_kind
    }

    /// Borrows the guaranteed participant metadata associated with this isolated audio stream.
    ///
    /// Inputs: None.
    /// Outputs: The immutable external participant.
    /// Side effects: None.
    pub fn external_participant(&self) -> &ExternalParticipant {
        &self.external_participant
    }

    /// Borrows opaque audio bytes for approved AI ports only.
    ///
    /// Inputs: None.
    /// Outputs: Raw bridge-provided audio that callers must never log.
    /// Side effects: None.
    pub fn audio_bytes(&self) -> &[u8] {
        &self.audio_bytes
    }

    /// The chunk as a WAV file, for the speaker-separation port. Bridges deliver 16 kHz mono
    /// 16-bit little-endian PCM (`EXTERNAL_AUDIO_SAMPLE_RATE_HZ`).
    ///
    /// Inputs: None. Outputs: The samples behind a WAV header; never log them.
    /// Side effects: None.
    pub fn wav_bytes(&self) -> Vec<u8> {
        crate::domain::pcm_wav::wav_from_pcm16_le(&self.audio_bytes, EXTERNAL_AUDIO_SAMPLE_RATE_HZ)
    }

    /// Returns the bridge timestamp for deterministic ordering and recording metadata.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds relative to the bridge meeting stream.
    /// Side effects: None.
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }
}

/// Which text an online-meeting service produced for a meeting (Issue #102).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeetingDocumentKind {
    /// The transcript Teams publishes after the meeting.
    TeamsTranscript,
    /// The Teams transcript panel, as read during the meeting.
    TeamsLiveTranscript,
    /// The Teams live captions, as read during the meeting.
    TeamsCaptions,
}

impl MeetingDocumentKind {
    /// A short file-name stem for the stored document. Side effects: None.
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::TeamsTranscript => "teams-transcript",
            Self::TeamsLiveTranscript => "teams-live-transcript",
            Self::TeamsCaptions => "teams-captions",
        }
    }
}

/// The text format of a meeting document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeetingDocumentMediaType {
    /// WebVTT (`text/vtt`).
    WebVtt,
    /// Plain UTF-8 text (`text/plain`).
    PlainText,
}

impl MeetingDocumentMediaType {
    /// The MIME type. Side effects: None.
    pub fn mime_type(self) -> &'static str {
        match self {
            Self::WebVtt => "text/vtt",
            Self::PlainText => "text/plain",
        }
    }

    /// The file extension, without the dot. Side effects: None.
    pub fn extension(self) -> &'static str {
        match self {
            Self::WebVtt => "vtt",
            Self::PlainText => "txt",
        }
    }
}

/// A text document an online-meeting service produced, to be stored with its meeting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMeetingDocument {
    meeting_id: Uuid,
    kind: MeetingDocumentKind,
    media_type: MeetingDocumentMediaType,
    text: String,
    timestamp_ms: u64,
}

impl ExternalMeetingDocument {
    /// Creates a document of an external meeting.
    ///
    /// Inputs: The meeting UUID, the kind, the format, the text (sensitive: never logged) and
    /// the meeting time it belongs at.
    /// Outputs: The document. Side effects: None.
    pub fn new(
        meeting_id: Uuid,
        kind: MeetingDocumentKind,
        media_type: MeetingDocumentMediaType,
        text: String,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            meeting_id,
            kind,
            media_type,
            text,
            timestamp_ms,
        }
    }

    /// The meeting UUID. Side effects: None.
    pub fn meeting_id(&self) -> Uuid {
        self.meeting_id
    }

    /// What the document is. Side effects: None.
    pub fn kind(&self) -> MeetingDocumentKind {
        self.kind
    }

    /// Its format. Side effects: None.
    pub fn media_type(&self) -> MeetingDocumentMediaType {
        self.media_type
    }

    /// Its text, which must never be logged. Side effects: None.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The meeting time it belongs at. Side effects: None.
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }
}

/// Carries an external screen-share frame for attachment workflow processing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlideCapture {
    meeting_id: Uuid,
    image_bytes: Vec<u8>,
    timestamp_ms: u64,
    source: String,
}

impl SlideCapture {
    /// Creates one opaque external slide frame associated with a meeting UUID.
    ///
    /// Inputs: `meeting_id` identifies the external meeting; `image_bytes` are sensitive capture
    /// data; `timestamp_ms` orders the capture; `source` identifies the adapter-neutral origin.
    /// Outputs: An immutable slide-capture value.
    /// Side effects: Takes ownership of image bytes without logging or persisting them.
    pub fn new(
        meeting_id: Uuid,
        image_bytes: Vec<u8>,
        timestamp_ms: u64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            meeting_id,
            image_bytes,
            timestamp_ms,
            source: source.into(),
        }
    }

    /// Returns the source meeting UUID from the external bridge.
    ///
    /// Inputs: None.
    /// Outputs: The meeting UUID.
    /// Side effects: None.
    pub fn meeting_id(&self) -> Uuid {
        self.meeting_id
    }

    /// Borrows image bytes for the attachment workflow without copying them.
    ///
    /// Inputs: None.
    /// Outputs: Opaque frame bytes that callers must never log.
    /// Side effects: None.
    pub fn image_bytes(&self) -> &[u8] {
        &self.image_bytes
    }

    /// Returns the source-relative capture timestamp.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds relative to the external meeting stream.
    /// Side effects: None.
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    /// Borrows the adapter-neutral source label.
    ///
    /// Inputs: None.
    /// Outputs: The source label supplied by the bridge adapter.
    /// Side effects: None.
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Carries one derived voice-signature variant without retaining raw audio.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceEmbedding {
    identity_id: Uuid,
    variant_id: Uuid,
    vector: Vec<f32>,
}

impl VoiceEmbedding {
    /// Creates a derived biometric signature variant for an existing speaker identity.
    ///
    /// Inputs: `identity_id` owns the variant; `variant_id` uniquely identifies this acoustic
    /// environment; `vector` contains model-derived numeric features and never raw audio.
    /// Outputs: An immutable embedding value suitable only for the voice-vector storage port.
    /// Side effects: Takes ownership of the biometric vector; callers must not log the value.
    pub fn new(identity_id: Uuid, variant_id: Uuid, vector: Vec<f32>) -> Self {
        Self {
            identity_id,
            variant_id,
            vector,
        }
    }

    /// Returns the UUID of the identity that owns this signature variant.
    ///
    /// Inputs: None.
    /// Outputs: The owning identity UUID by value.
    /// Side effects: None.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// Returns the UUID that distinguishes this acoustic signature from sibling variants.
    ///
    /// Inputs: None.
    /// Outputs: The variant UUID by value.
    /// Side effects: None.
    pub fn variant_id(&self) -> Uuid {
        self.variant_id
    }

    /// Borrows the model-derived vector without copying sensitive biometric values.
    ///
    /// Inputs: None.
    /// Outputs: The derived embedding dimensions.
    /// Side effects: None.
    pub fn vector(&self) -> &[f32] {
        &self.vector
    }
}

/// Reports the confidence category returned after a voice-vector similarity search.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceMatchResult {
    /// A high-confidence candidate can be attributed without user confirmation.
    StrongMatch(Uuid),
    /// A plausible candidate requires user confirmation and reports its adapter-provided score.
    WeakMatch(Uuid, f32),
    /// No candidate is sufficiently similar to attribute safely.
    NoMatch,
}

/// Retains the candidate metadata needed to safely confirm a deferred weak match.
#[derive(Clone, Debug, PartialEq)]
pub struct WeakVoiceMatchCandidate {
    identity_id: Uuid,
    confidence_score: f32,
}

impl WeakVoiceMatchCandidate {
    /// Creates a weak candidate returned by the existing identity-match policy.
    ///
    /// Inputs: `identity_id` identifies the candidate and `confidence_score` is the adapter score.
    /// Outputs: Immutable metadata that contains no embedding or raw audio.
    /// Side effects: None.
    pub fn new(identity_id: Uuid, confidence_score: f32) -> Self {
        Self {
            identity_id,
            confidence_score,
        }
    }

    /// Returns the candidate identity UUID.
    ///
    /// Inputs: None.
    /// Outputs: The suggested identity by value.
    /// Side effects: None.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// Returns the adapter-provided weak-match confidence.
    ///
    /// Inputs: None.
    /// Outputs: The score in the matching adapter's normalized range.
    /// Side effects: None.
    pub fn confidence_score(&self) -> f32 {
        self.confidence_score
    }
}

/// Retains a temporary, reviewable audio payload for a diarized segment without exposing storage details.
#[derive(Clone, Debug, PartialEq)]
pub struct UnassignedVoiceClip {
    clip_id: Uuid,
    meeting_id: MeetingId,
    start_time_ms: u64,
    end_time_ms: u64,
    audio_bytes: Vec<u8>,
    derived_embedding: Vec<f32>,
    weak_match_candidate: Option<WeakVoiceMatchCandidate>,
    /// Unknown-speaker cluster this clip represents; `None` for unclustered legacy clips.
    cluster_id: Option<Uuid>,
}

impl UnassignedVoiceClip {
    /// Creates a temporary review clip for a segment that could not be attributed automatically.
    ///
    /// Inputs: `clip_id` uniquely identifies the review item; `meeting_id` owns it; the time range
    /// identifies the segment inside `audio_bytes`, which is retained only until the user resolves it.
    /// Outputs: An immutable clip value suitable for persistence through `NoteRepository`.
    /// Side effects: Takes ownership of raw audio bytes; callers must not log the returned value.
    pub fn new(
        clip_id: Uuid,
        meeting_id: MeetingId,
        start_time_ms: u64,
        end_time_ms: u64,
        audio_bytes: Vec<u8>,
    ) -> Self {
        Self {
            clip_id,
            meeting_id,
            start_time_ms,
            end_time_ms,
            audio_bytes,
            derived_embedding: Vec::new(),
            weak_match_candidate: None,
            cluster_id: None,
        }
    }

    /// Creates a review clip retaining derived confirmation context from diarization.
    ///
    /// Inputs: `derived_embedding` is a model feature vector and `weak_match_candidate` is the
    /// optional policy result; neither value may be logged or exposed through FFI.
    /// Outputs: A clip that permits later application-level assignment or confirmation.
    /// Side effects: Takes ownership of temporary audio and sensitive derived embedding data.
    pub fn with_review_context(
        clip_id: Uuid,
        meeting_id: MeetingId,
        start_time_ms: u64,
        end_time_ms: u64,
        audio_bytes: Vec<u8>,
        derived_embedding: Vec<f32>,
        weak_match_candidate: Option<WeakVoiceMatchCandidate>,
    ) -> Self {
        Self {
            clip_id,
            meeting_id,
            start_time_ms,
            end_time_ms,
            audio_bytes,
            derived_embedding,
            weak_match_candidate,
            cluster_id: None,
        }
    }

    /// Links the clip to the unknown-speaker cluster it represents in the review workflow.
    ///
    /// Inputs: `cluster_id` identifies an `UnknownSpeakerCluster` of the same meeting, or `None`.
    /// Outputs: The same clip carrying the cluster link.
    /// Side effects: None.
    pub fn with_cluster_id(mut self, cluster_id: Option<Uuid>) -> Self {
        self.cluster_id = cluster_id;
        self
    }

    /// Returns the UUID assigned to this temporary manual-review item.
    ///
    /// Inputs: None.
    /// Outputs: The globally unique clip identifier.
    /// Side effects: None.
    pub fn clip_id(&self) -> Uuid {
        self.clip_id
    }

    /// Returns the meeting that owns this temporary review clip.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the owning meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the segment start timestamp relative to the original recording.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds from the start of the processed audio.
    /// Side effects: None.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the segment end timestamp relative to the original recording.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds from the start of the processed audio.
    /// Side effects: None.
    pub fn end_time_ms(&self) -> u64 {
        self.end_time_ms
    }

    /// Borrows the temporarily retained source audio without copying it.
    ///
    /// Inputs: None.
    /// Outputs: Opaque raw audio bytes for a storage adapter or manual-review playback workflow.
    /// Side effects: None; callers must not log the returned bytes.
    pub fn audio_bytes(&self) -> &[u8] {
        &self.audio_bytes
    }

    /// Returns the audio to play when the owner reviews this voice: only its span (#116).
    ///
    /// Clips saved before #116 hold a whole chunk, often with several people, and mark the voice
    /// with `start_time_ms`..`end_time_ms`; newer clips already hold just that span.
    ///
    /// Inputs: None.
    /// Outputs: A WAV of the span, or the stored audio unchanged when it is not a mono 16-bit WAV
    /// or the span holds no sample.
    /// Side effects: None; callers must not log the returned bytes.
    pub fn voice_audio(&self) -> std::borrow::Cow<'_, [u8]> {
        match crate::domain::pcm_wav::slice_pcm16_wav(
            &self.audio_bytes,
            self.start_time_ms,
            self.end_time_ms,
        ) {
            Some(span_audio) => std::borrow::Cow::Owned(span_audio),
            None => std::borrow::Cow::Borrowed(&self.audio_bytes),
        }
    }

    /// Borrows the derived biometric embedding retained solely for user-confirmed assignment.
    ///
    /// Inputs: None.
    /// Outputs: Model-derived dimensions or an empty slice for legacy clips.
    /// Side effects: None; callers must never log this biometric value.
    pub fn derived_embedding(&self) -> &[f32] {
        &self.derived_embedding
    }

    /// Borrows weak-match candidate metadata, if diarization produced a candidate.
    ///
    /// Inputs: None.
    /// Outputs: Candidate UUID and score without biometric vector data.
    /// Side effects: None.
    pub fn weak_match_candidate(&self) -> Option<&WeakVoiceMatchCandidate> {
        self.weak_match_candidate.as_ref()
    }

    /// Returns the unknown-speaker cluster this clip represents, if it was clustered.
    ///
    /// Inputs: None.
    /// Outputs: The cluster UUID, or `None` for clips saved without clustering.
    /// Side effects: None.
    pub fn cluster_id(&self) -> Option<Uuid> {
        self.cluster_id
    }
}

/// Groups one meeting's unknown-voice chunks that are acoustically similar enough to be one person.
///
/// The representative embedding is the running mean of every member chunk embedding. It is
/// derived biometric data and must never be logged or exposed through FFI/HTTP.
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownSpeakerCluster {
    cluster_id: Uuid,
    meeting_id: MeetingId,
    representative_embedding: Vec<f32>,
    chunk_ids: Vec<String>,
    total_duration_ms: u64,
    created_at: DateTime<Utc>,
    /// Identity the user assigned to the cluster; `None` while it awaits review.
    resolved_identity_id: Option<Uuid>,
}

impl UnknownSpeakerCluster {
    /// Creates a cluster value from its persisted or freshly computed parts.
    ///
    /// Inputs: `cluster_id` is unique; `meeting_id` owns the cluster; `representative_embedding`
    /// is the mean of member embeddings; `chunk_ids` lists members in insertion order;
    /// `total_duration_ms` sums member durations; `resolved_identity_id` is set once assigned.
    /// Outputs: An immutable cluster value.
    /// Side effects: Takes ownership of the biometric representative vector; callers must not log it.
    pub fn new(
        cluster_id: Uuid,
        meeting_id: MeetingId,
        representative_embedding: Vec<f32>,
        chunk_ids: Vec<String>,
        total_duration_ms: u64,
        created_at: DateTime<Utc>,
        resolved_identity_id: Option<Uuid>,
    ) -> Self {
        Self {
            cluster_id,
            meeting_id,
            representative_embedding,
            chunk_ids,
            total_duration_ms,
            created_at,
            resolved_identity_id,
        }
    }

    /// Returns the cluster UUID.
    pub fn cluster_id(&self) -> Uuid {
        self.cluster_id
    }

    /// Borrows the owning meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Borrows the running-mean embedding; callers must never log it.
    pub fn representative_embedding(&self) -> &[f32] {
        &self.representative_embedding
    }

    /// Borrows member chunk identifiers in insertion order.
    pub fn chunk_ids(&self) -> &[String] {
        &self.chunk_ids
    }

    /// Returns the summed duration of every member chunk in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_ms
    }

    /// Returns the UTC instant at which the first member chunk created the cluster.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the identity assigned by the user, or `None` while unresolved.
    pub fn resolved_identity_id(&self) -> Option<Uuid> {
        self.resolved_identity_id
    }

    /// Reports whether the user has already assigned an identity to this cluster.
    pub fn is_resolved(&self) -> bool {
        self.resolved_identity_id.is_some()
    }
}

/// One unknown-voice chunk recorded as a member of an `UnknownSpeakerCluster`.
///
/// Its embedding is kept so every member can be enrolled as a voice variant on assignment; it is
/// derived biometric data, never raw audio, and must never be logged.
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownSpeakerChunk {
    chunk_id: String,
    embedding: Vec<f32>,
    duration_ms: u64,
}

impl UnknownSpeakerChunk {
    /// Creates a cluster member.
    ///
    /// Inputs: `chunk_id` is unique within the meeting (the pipeline uses the review-clip UUID);
    /// `embedding` is the diarization-derived vector; `duration_ms` is the speaker segment length.
    /// Outputs: An immutable member value.
    /// Side effects: Takes ownership of the biometric vector; callers must not log it.
    pub fn new(chunk_id: impl Into<String>, embedding: Vec<f32>, duration_ms: u64) -> Self {
        Self {
            chunk_id: chunk_id.into(),
            embedding,
            duration_ms,
        }
    }

    /// Borrows the chunk identifier.
    pub fn chunk_id(&self) -> &str {
        &self.chunk_id
    }

    /// Borrows the derived embedding; callers must never log it.
    pub fn embedding(&self) -> &[f32] {
        &self.embedding
    }

    /// Returns the chunk duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

/// An unresolved unknown-speaker cluster as copied to a paired device (Issue #50): the cluster,
/// its member chunks with their embeddings, and some or all of its review clips.
///
/// It holds biometric embeddings and raw audio; callers must never log it, and it only travels
/// sealed with the pair key.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterShare {
    cluster: UnknownSpeakerCluster,
    chunks: Vec<UnknownSpeakerChunk>,
    clips: Vec<UnassignedVoiceClip>,
}

impl ClusterShare {
    /// Groups a cluster with its members and clips.
    ///
    /// Inputs: An unresolved cluster, its member chunks in insertion order, and review clips
    /// linked to it (a share may carry only some clips; others travel in further shares).
    /// Outputs: The share. Side effects: Takes ownership of sensitive data.
    pub fn new(
        cluster: UnknownSpeakerCluster,
        chunks: Vec<UnknownSpeakerChunk>,
        clips: Vec<UnassignedVoiceClip>,
    ) -> Self {
        Self {
            cluster,
            chunks,
            clips,
        }
    }

    /// Borrows the cluster.
    pub fn cluster(&self) -> &UnknownSpeakerCluster {
        &self.cluster
    }

    /// Borrows the member chunks.
    pub fn chunks(&self) -> &[UnknownSpeakerChunk] {
        &self.chunks
    }

    /// Borrows the review clips this share carries.
    pub fn clips(&self) -> &[UnassignedVoiceClip] {
        &self.clips
    }
}

/// What an unresolved cluster holds, so a device can tell which clusters changed since it last
/// shared them (Issue #50). Chunks and clips are only ever added while a cluster is unresolved.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ClusterDigest {
    cluster_id: Uuid,
    chunk_count: usize,
    clip_count: usize,
    has_final_voice: bool,
}

/// Names the member that holds a cluster's final whole-meeting voice.
///
/// Inputs: The cluster. Outputs: The chunk ID. Side effects: None.
pub fn meeting_voice_chunk_id(cluster_id: Uuid) -> String {
    format!("meeting-voice-{cluster_id}")
}

impl ClusterDigest {
    /// Creates a digest.
    ///
    /// Inputs: The cluster and how many member chunks and review clips it holds.
    /// Outputs: The digest. Side effects: None.
    pub fn new(cluster_id: Uuid, chunk_count: usize, clip_count: usize) -> Self {
        Self {
            cluster_id,
            chunk_count,
            clip_count,
            has_final_voice: false,
        }
    }

    /// Records whether the cluster's member is the final whole-meeting voice, which replaces live
    /// members and so can change the cluster without changing the counts.
    ///
    /// Inputs: Whether it is. Outputs: The digest. Side effects: None.
    pub fn with_final_voice(mut self, has_final_voice: bool) -> Self {
        self.has_final_voice = has_final_voice;
        self
    }

    /// The cluster.
    pub fn cluster_id(&self) -> Uuid {
        self.cluster_id
    }

    /// How many member chunks it holds.
    pub fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    /// How many review clips it holds.
    pub fn clip_count(&self) -> usize {
        self.clip_count
    }
}

/// What importing a `ClusterShare` from a paired device did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClusterShareImport {
    /// The cluster and whatever chunks and clips were new are stored.
    Imported,
    /// The cluster is already resolved here; nothing was stored, so no audio comes back.
    AlreadyResolved,
    /// The meeting has not synced to this device yet; nothing was stored.
    UnknownMeeting,
    /// The voice or its meeting was deleted; nothing was stored, and it never will be.
    Deleted,
}

/// What this device knows was deleted, on it or on a paired device. Holds identifiers only.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeletionRecords {
    /// Deleted meetings, which sync must not restore.
    pub meeting_ids: Vec<MeetingId>,
    /// Deleted unknown voices, which cluster shares must not restore.
    pub cluster_ids: Vec<Uuid>,
    /// Deleted project resources (Issue #85), which attachment sync must not restore.
    pub attachment_ids: Vec<Uuid>,
}

/// What applying a paired device's deletion records removed here, so the caller can remove the
/// files that belonged to it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppliedDeletions {
    /// Meetings removed by this call.
    pub meeting_ids: Vec<MeetingId>,
    /// Project resources removed by this call, with the project each belonged to.
    pub resources: Vec<(Uuid, crate::domain::projects::ProjectId)>,
}

impl Transcript {
    /// Creates transcript content associated with its owning meeting.
    ///
    /// Inputs: `content` is normalized text returned by an AI adapter.
    /// Outputs: An immutable transcript ready for persistence.
    /// Side effects: None.
    pub fn new(meeting_id: MeetingId, content: impl Into<String>) -> Self {
        Self {
            meeting_id,
            content: content.into(),
            timed_words: Vec::new(),
        }
    }

    /// Attaches the timing of each word, for engines that can report it.
    ///
    /// Inputs: The transcript's words in spoken order, or nothing when timing is unknown.
    /// Outputs: The transcript.
    /// Side effects: None.
    pub fn with_timed_words(mut self, timed_words: Vec<TimedWord>) -> Self {
        self.timed_words = timed_words;
        self
    }

    /// Borrows the timed words; empty when the engine reported no timing.
    pub fn timed_words(&self) -> &[TimedWord] {
        &self.timed_words
    }

    /// Returns the meeting that owns this transcript.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the transcript's normalized text.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of transcript content.
    /// Side effects: None.
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Represents one persisted, time-bounded transcript interval with optional speaker attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptSegment {
    meeting_id: MeetingId,
    start_time_ms: u64,
    end_time_ms: u64,
    /// `None` when diarization could not attribute the interval to a known identity.
    speaker_identity_id: Option<Uuid>,
    text: String,
    /// Unknown-speaker cluster the interval was grouped into, used to relink it on assignment.
    unknown_cluster_id: Option<Uuid>,
    /// How doubtful the interval is: overlap, an uncertain speaker, uncertain words (#90, #91).
    confidence: crate::domain::transcript_confidence::SegmentConfidence,
}

impl TranscriptSegment {
    /// Creates a transcript segment owned by one meeting.
    ///
    /// Inputs: `start_time_ms`/`end_time_ms` are offsets from the meeting start (end should not
    /// precede start); `speaker_identity_id` references a `SpeakerIdentity` when attributed.
    /// Outputs: An immutable segment ready for persistence or export.
    /// Side effects: None.
    pub fn new(
        meeting_id: MeetingId,
        start_time_ms: u64,
        end_time_ms: u64,
        speaker_identity_id: Option<Uuid>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            meeting_id,
            start_time_ms,
            end_time_ms,
            speaker_identity_id,
            text: text.into(),
            unknown_cluster_id: None,
            confidence: Default::default(),
        }
    }

    /// Replaces the text after the owner corrects it (#96), keeping speaker, timing and the
    /// overlap and speaker marks; the uncertain-word marks described the old words, so they go.
    ///
    /// Inputs: The new text. Outputs: The segment. Side effects: None.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.confidence.uncertain_words.clear();
        self
    }

    /// Attaches how doubtful the segment is (#90, #91).
    ///
    /// Inputs: The marks; word positions refer to this segment's text.
    /// Outputs: The same segment carrying them. Side effects: None.
    pub fn with_confidence(
        mut self,
        confidence: crate::domain::transcript_confidence::SegmentConfidence,
    ) -> Self {
        self.confidence = confidence;
        self
    }

    /// Borrows how doubtful the segment is.
    pub fn confidence(&self) -> &crate::domain::transcript_confidence::SegmentConfidence {
        &self.confidence
    }

    /// Links the segment to the unknown-speaker cluster its voice was grouped into.
    ///
    /// Inputs: `unknown_cluster_id` identifies a cluster of the same meeting, or `None`.
    /// Outputs: The same segment carrying the cluster link.
    /// Side effects: None.
    pub fn with_unknown_cluster_id(mut self, unknown_cluster_id: Option<Uuid>) -> Self {
        self.unknown_cluster_id = unknown_cluster_id;
        self
    }

    /// Returns a copy attributed to `speaker_identity_id`, keeping text, timing, and cluster link.
    ///
    /// Inputs: `speaker_identity_id` is the identity the user assigned.
    /// Outputs: The re-attributed segment.
    /// Side effects: None.
    pub fn with_speaker_identity_id(mut self, speaker_identity_id: Option<Uuid>) -> Self {
        self.speaker_identity_id = speaker_identity_id;
        self
    }

    /// Borrows the owning meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the start offset from the meeting start in milliseconds.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the end offset from the meeting start in milliseconds.
    pub fn end_time_ms(&self) -> u64 {
        self.end_time_ms
    }

    /// Returns the attributed speaker identity, if diarization produced a strong match.
    pub fn speaker_identity_id(&self) -> Option<Uuid> {
        self.speaker_identity_id
    }

    /// Borrows the segment's transcript text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the unknown-speaker cluster this segment belongs to, if it was clustered.
    pub fn unknown_cluster_id(&self) -> Option<Uuid> {
        self.unknown_cluster_id
    }
}

/// A validated, human-readable meeting name (Issue #35).
///
/// Invariant: trimmed, non-empty, at most `MAX_CHARACTERS` Unicode scalar values, and free of
/// control characters, so it is safe to print in a terminal and to embed in export headers.
/// Titles are user content: adapters must not log them.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MeetingTitle(String);

impl MeetingTitle {
    /// Longest accepted title, counted in Unicode scalar values after trimming.
    pub const MAX_CHARACTERS: usize = 200;

    /// Validates user-supplied title text.
    ///
    /// Inputs: `raw_title` from a CLI flag, FFI string, HTTP body, or the Teams metadata; leading
    /// and trailing whitespace is ignored.
    /// Outputs: The trimmed title, or `Blank`, `TooLong`, or `ControlCharacter`.
    /// Side effects: None.
    pub fn parse(raw_title: &str) -> Result<Self, MeetingTitleError> {
        let trimmed_title = raw_title.trim();
        if trimmed_title.is_empty() {
            return Err(MeetingTitleError::Blank);
        }
        if trimmed_title.chars().count() > Self::MAX_CHARACTERS {
            return Err(MeetingTitleError::TooLong {
                max_characters: Self::MAX_CHARACTERS,
            });
        }
        if trimmed_title.chars().any(char::is_control) {
            return Err(MeetingTitleError::ControlCharacter);
        }
        Ok(Self(trimmed_title.to_owned()))
    }

    /// Borrows the title text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Describes repository-level facts about a meeting that are independent of its content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingMetadata {
    meeting_id: MeetingId,
    created_at: DateTime<Utc>,
    /// The user-chosen title; `None` for meetings created without one (including every meeting
    /// stored before Issue #35).
    title: Option<MeetingTitle>,
    /// The languages its speech was transcribed in, in order of first use; empty for meetings
    /// recorded before they were kept, or transcribed without a chosen language.
    transcription_languages: Vec<TranscriptionLanguage>,
}

impl MeetingMetadata {
    /// Creates meeting metadata from persisted repository values.
    ///
    /// Inputs: `meeting_id` identifies the meeting; `created_at` is its UTC creation instant.
    /// Outputs: Immutable metadata.
    /// Side effects: None.
    pub fn new(meeting_id: MeetingId, created_at: DateTime<Utc>) -> Self {
        Self {
            meeting_id,
            created_at,
            title: None,
            transcription_languages: Vec::new(),
        }
    }

    /// Attaches the languages the meeting was transcribed in.
    ///
    /// Inputs: The languages in order of first use. Outputs: The metadata carrying them.
    /// Side effects: None.
    pub fn with_transcription_languages(
        mut self,
        transcription_languages: Vec<TranscriptionLanguage>,
    ) -> Self {
        self.transcription_languages = transcription_languages;
        self
    }

    /// The languages the meeting's speech was transcribed in, in order of first use: usually
    /// one, more when it was changed during the recording, none when unknown.
    pub fn transcription_languages(&self) -> &[TranscriptionLanguage] {
        &self.transcription_languages
    }

    /// Attaches the persisted title.
    ///
    /// Inputs: The stored title, or `None` for an untitled meeting.
    /// Outputs: The metadata carrying that title.
    /// Side effects: None.
    pub fn with_title(mut self, title: Option<MeetingTitle>) -> Self {
        self.title = title;
        self
    }

    /// Borrows the meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the UTC instant at which the meeting record was created.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Borrows the user-chosen title, if one was stored.
    pub fn title(&self) -> Option<&MeetingTitle> {
        self.title.as_ref()
    }

    /// Returns the name shown to people: the stored title, or for an untitled meeting a fallback
    /// derived from its creation time, such as `Meeting 2026-10-27 09:05 UTC`.
    ///
    /// Inputs: None.
    /// Outputs: A non-empty display name. The fallback is computed on read and never persisted.
    /// Side effects: None.
    pub fn display_title(&self) -> String {
        match &self.title {
            Some(title) => title.as_str().to_owned(),
            None => format!("Meeting {}", self.created_at.format("%Y-%m-%d %H:%M UTC")),
        }
    }
}

#[cfg(test)]
mod meeting_title_tests {
    //! Specifies meeting-title validation and the display fallback for untitled meetings.

    use chrono::{TimeZone, Utc};

    use super::{MeetingId, MeetingMetadata, MeetingTitle};
    use crate::domain::errors::MeetingTitleError;

    /// Verifies titles are trimmed and blank, oversized, or control-character titles rejected.
    #[test]
    fn meeting_titles_are_trimmed_and_validated() {
        let parsed_title = MeetingTitle::parse("  Project Sync \n")
            .unwrap_or_else(|error| panic!("title must parse: {error}"));
        assert_eq!(parsed_title.as_str(), "Project Sync");

        assert_eq!(MeetingTitle::parse(" \t "), Err(MeetingTitleError::Blank));
        assert_eq!(
            MeetingTitle::parse(&"x".repeat(MeetingTitle::MAX_CHARACTERS + 1)),
            Err(MeetingTitleError::TooLong {
                max_characters: MeetingTitle::MAX_CHARACTERS
            })
        );
        assert!(MeetingTitle::parse(&"é".repeat(MeetingTitle::MAX_CHARACTERS)).is_ok());
        assert_eq!(
            MeetingTitle::parse("Sync\u{0007}"),
            Err(MeetingTitleError::ControlCharacter)
        );
    }

    /// Verifies a stored title is displayed as is, and an untitled meeting gets a dated fallback.
    #[test]
    fn display_title_falls_back_to_the_creation_time() {
        let created_at = Utc
            .with_ymd_and_hms(2026, 10, 27, 9, 5, 0)
            .single()
            .unwrap_or_else(|| panic!("fixture time must be valid"));
        let untitled_metadata = MeetingMetadata::new(MeetingId::new("meeting-1"), created_at);
        assert_eq!(untitled_metadata.title(), None);
        assert_eq!(
            untitled_metadata.display_title(),
            "Meeting 2026-10-27 09:05 UTC"
        );

        let title = MeetingTitle::parse("Project Sync")
            .unwrap_or_else(|error| panic!("title must parse: {error}"));
        let titled_metadata = untitled_metadata.with_title(Some(title.clone()));
        assert_eq!(titled_metadata.title(), Some(&title));
        assert_eq!(titled_metadata.display_title(), "Project Sync");
    }
}
