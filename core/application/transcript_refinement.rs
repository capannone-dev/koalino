//! Re-transcribes a finished meeting in long windows and keeps its speakers (#87).
//!
//! Live transcription cuts speech into short chunks (about 11 s), so Whisper hears little context
//! and words at the cuts suffer. After the meeting, the speech chunks are joined into windows of up
//! to 30 s, the length Whisper was trained on, and transcribed again, possibly with a larger model.
//! Each new word takes the speaker the final speaker pass gave that moment, so names and unknown
//! voices are unchanged and only the text improves. Planning and attribution are pure;
//! `refine_meeting` drives an engine and a repository through their ports.

use uuid::Uuid;

use crate::{
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, TranscriptSegment,
            TranscriptionContext,
        },
        transcription_language::TranscriptionLanguage,
    },
    ports::{ai::TranscriptionEngine, storage::NoteRepository},
};

/// Longest window transcribed at once: Whisper's own input length.
pub const REFINEMENT_WINDOW_MS: u64 = 30_000;
/// A pause longer than this ends a segment even when the speaker stays.
const SEGMENT_PAUSE_MS: u64 = 2_000;
/// No segment grows past this length, so lines stay readable.
const SEGMENT_MAX_MS: u64 = 20_000;

/// One speech chunk's place in the meeting, in milliseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkSpan {
    /// Start in meeting time.
    pub start_ms: u64,
    /// End in meeting time; not before `start_ms`.
    pub end_ms: u64,
}

impl ChunkSpan {
    /// Returns the chunk's length.
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

/// One stretch of a window that comes from one chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WindowPiece {
    /// Where the piece starts within the window.
    window_start_ms: u64,
    /// Where the same audio starts in the meeting.
    meeting_start_ms: u64,
    /// Its length.
    duration_ms: u64,
}

/// Consecutive speech chunks joined end to end and transcribed together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefinementWindow {
    chunk_indexes: Vec<usize>,
    pieces: Vec<WindowPiece>,
}

impl RefinementWindow {
    /// Returns the chunks joined, as indexes into the planned list, in order.
    pub fn chunk_indexes(&self) -> &[usize] {
        &self.chunk_indexes
    }

    /// Returns the window's length: the sum of its chunks.
    pub fn duration_ms(&self) -> u64 {
        self.pieces
            .last()
            .map_or(0, |piece| piece.window_start_ms + piece.duration_ms)
    }

    /// Converts a time within the window to meeting time.
    ///
    /// Inputs: Milliseconds from the window's start. Outputs: The meeting time of the same audio;
    /// a time past the window's end maps to the last chunk's end. Side effects: None.
    pub fn to_meeting_ms(&self, window_ms: u64) -> u64 {
        let position = self
            .pieces
            .partition_point(|piece| piece.window_start_ms <= window_ms)
            .saturating_sub(1);
        self.pieces.get(position).map_or(0, |piece| {
            piece.meeting_start_ms + (window_ms - piece.window_start_ms).min(piece.duration_ms)
        })
    }
}

/// Joins consecutive chunks into windows no longer than `max_window_ms`.
///
/// Inputs: The meeting's speech chunks in time order, and the longest window.
/// Outputs: Windows in order covering every chunk once; a chunk longer than the limit stands
/// alone. Side effects: None.
pub fn plan_windows(spans: &[ChunkSpan], max_window_ms: u64) -> Vec<RefinementWindow> {
    let mut windows: Vec<RefinementWindow> = Vec::new();
    for (index, span) in spans.iter().enumerate() {
        let fits = windows
            .last()
            .is_some_and(|window| window.duration_ms() + span.duration_ms() <= max_window_ms);
        if !fits {
            windows.push(RefinementWindow {
                chunk_indexes: Vec::new(),
                pieces: Vec::new(),
            });
        }
        if let Some(window) = windows.last_mut() {
            let window_start_ms = window.duration_ms();
            window.chunk_indexes.push(index);
            window.pieces.push(WindowPiece {
                window_start_ms,
                meeting_start_ms: span.start_ms,
                duration_ms: span.duration_ms(),
            });
        }
    }
    windows
}

/// One re-transcribed word in meeting time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingWord {
    text: String,
    start_ms: u64,
    end_ms: u64,
}

impl MeetingWord {
    /// Creates a word; an end before the start is raised to the start.
    pub fn new(text: impl Into<String>, start_ms: u64, end_ms: u64) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms: end_ms.max(start_ms),
        }
    }

    /// Returns the moment that decides who said the word.
    fn midpoint_ms(&self) -> u64 {
        self.start_ms + (self.end_ms - self.start_ms) / 2
    }
}

/// Who a stretch of the meeting belongs to: a named person, an unknown voice, or neither.
type Speaker = (Option<Uuid>, Option<Uuid>);

/// Returns the speaker of the segment containing `time_ms`, else of the nearest segment.
fn speaker_at(speakers: &[TranscriptSegment], time_ms: u64) -> Speaker {
    speakers
        .iter()
        .min_by_key(|segment| {
            if time_ms < segment.start_time_ms() {
                segment.start_time_ms() - time_ms
            } else {
                time_ms.saturating_sub(segment.end_time_ms())
            }
        })
        .map_or((None, None), |segment| {
            (segment.speaker_identity_id(), segment.unknown_cluster_id())
        })
}

/// Builds the meeting's new segments from re-transcribed words and the final pass's speakers.
///
/// Inputs: The meeting, its new words in time order, and the segments standing now (their text is
/// ignored; only who spoke when is used).
/// Outputs: Segments of consecutive words with one speaker, split at pauses over 2 s and kept
/// within 20 s; blank words are dropped. Side effects: None.
pub fn reattribute_words(
    meeting_id: &MeetingId,
    words: &[MeetingWord],
    speakers: &[TranscriptSegment],
) -> Vec<TranscriptSegment> {
    let mut runs: Vec<(Speaker, u64, u64, Vec<&str>)> = Vec::new();
    for word in words {
        let text = word.text.trim();
        if text.is_empty() {
            continue;
        }
        let speaker = speaker_at(speakers, word.midpoint_ms());
        match runs.last_mut() {
            Some(run)
                if run.0 == speaker
                    && word.start_ms.saturating_sub(run.2) <= SEGMENT_PAUSE_MS
                    && word.end_ms.saturating_sub(run.1) <= SEGMENT_MAX_MS =>
            {
                run.2 = run.2.max(word.end_ms);
                run.3.push(text);
            }
            _ => runs.push((speaker, word.start_ms, word.end_ms, vec![text])),
        }
    }
    runs.into_iter()
        .map(|((identity_id, cluster_id), start_ms, end_ms, texts)| {
            TranscriptSegment::new(
                meeting_id.clone(),
                start_ms,
                end_ms,
                identity_id,
                texts.join(" "),
            )
            .with_unknown_cluster_id(cluster_id)
        })
        .collect()
}

/// One retained speech chunk handed to `refine_meeting`.
#[derive(Clone, Copy, Debug)]
pub struct RefinementChunk<'samples> {
    /// Where it sits in the meeting.
    pub span: ChunkSpan,
    /// The language it was recorded with, if any.
    pub language: Option<TranscriptionLanguage>,
    /// Its 16 kHz mono samples.
    pub samples: &'samples [i16],
}

/// What a finished pass did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefinementOutcome {
    /// Windows transcribed.
    pub window_count: usize,
    /// Segments now standing for the re-transcribed audio.
    pub segment_count: usize,
}

/// Why a pass stopped without changing the transcript.
#[derive(Debug, thiserror::Error)]
pub enum RefinementError {
    /// The caller asked it to stop (for example the phone was unplugged); it can run again later.
    #[error("the transcript pass was cancelled")]
    Cancelled,
    /// A window could not be transcribed.
    #[error("a window could not be transcribed: {0}")]
    Transcription(#[from] TranscriptionError),
    /// The transcript could not be read or saved.
    #[error("the transcript could not be read or saved: {0}")]
    Storage(#[from] StorageError),
}

/// Re-transcribes a finished meeting's retained speech and replaces the text it covers.
///
/// Inputs: The meeting; its retained chunks in time order, each with the language it was recorded
/// with (a window uses its first chunk's); the engine; `provide_audio`, which hands one window's
/// samples to the engine's audio source under the given id; the repository; this device (named as
/// the acquiring node); and `cancelled`, polled before each window.
/// Outputs: The outcome, or why nothing changed. Each window is prompted with the previous one's
/// text, as live transcription is. Segments whose middle lies outside every retained chunk (for
/// example recorded before retention was on) are kept as they are.
/// Side effects: Runs inference and rewrites the meeting's segments, which also updates the
/// meeting's synced document. Nothing is written until every window is done.
pub fn refine_meeting<Engine, Repository>(
    meeting_id: &MeetingId,
    chunks: &[RefinementChunk<'_>],
    engine: &Engine,
    provide_audio: &dyn Fn(&RecordingId, Vec<i16>),
    repository: &Repository,
    device_id: &DeviceId,
    cancelled: &dyn Fn() -> bool,
) -> Result<RefinementOutcome, RefinementError>
where
    Engine: TranscriptionEngine + ?Sized,
    Repository: NoteRepository + ?Sized,
{
    let spans: Vec<ChunkSpan> = chunks.iter().map(|chunk| chunk.span).collect();
    let windows = plan_windows(&spans, REFINEMENT_WINDOW_MS);
    let mut words: Vec<MeetingWord> = Vec::new();
    let mut previous_text: Option<String> = None;
    for (window_number, window) in windows.iter().enumerate() {
        if cancelled() {
            return Err(RefinementError::Cancelled);
        }
        let samples: Vec<i16> = window
            .chunk_indexes()
            .iter()
            .flat_map(|index| chunks[*index].samples.iter().copied())
            .collect();
        let language = window
            .chunk_indexes()
            .first()
            .and_then(|index| chunks[*index].language);
        let recording_id =
            RecordingId::new(format!("refine-{}-{window_number}", meeting_id.as_str()));
        provide_audio(&recording_id, samples);
        let context = TranscriptionContext::new(previous_text.take()).with_language(language);
        let transcript = engine.transcribe(
            &AudioRecording::new(recording_id, meeting_id.clone(), device_id.clone()),
            &context,
        )?;
        words.extend(transcript.timed_words().iter().map(|word| {
            MeetingWord::new(
                word.text(),
                window.to_meeting_ms(word.start_ms()),
                window.to_meeting_ms(word.end_ms()),
            )
        }));
        let text = transcript.content().trim();
        previous_text = (!text.is_empty()).then(|| text.to_owned());
    }
    if cancelled() {
        return Err(RefinementError::Cancelled);
    }
    let standing = repository.load_transcript_segments(meeting_id)?;
    let covered = |segment: &TranscriptSegment| {
        let middle = segment.start_time_ms()
            + segment
                .end_time_ms()
                .saturating_sub(segment.start_time_ms())
                / 2;
        spans
            .iter()
            .any(|span| span.start_ms <= middle && middle <= span.end_ms)
    };
    let (speakers, kept): (Vec<TranscriptSegment>, Vec<TranscriptSegment>) =
        standing.into_iter().partition(|segment| covered(segment));
    let refined = reattribute_words(meeting_id, &words, &speakers);
    let segment_count = refined.len();
    let mut timeline = kept;
    timeline.extend(refined);
    timeline.sort_by_key(TranscriptSegment::start_time_ms);
    repository.save_transcript_segments(meeting_id, &timeline)?;
    Ok(RefinementOutcome {
        window_count: windows.len(),
        segment_count,
    })
}
