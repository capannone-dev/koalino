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
            AudioRecording, DeviceId, MeetingId, RecordingId, SegmentSource, TranscriptSegment,
            TranscriptionContext,
        },
        transcript_confidence::{
            OverlapSpan, SegmentConfidence, is_uncertain_probability, mark_overlapping_segments,
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
/// Longest a word may last. Whisper's word times often run on to the next word, so a word before
/// a silence seems to last through it, and in a 30 s window that silence can be long. Left as is,
/// it moved the word's midpoint into the next speaker's turn and stretched segments over silence:
/// on the AMI test meetings the pass raised speaker error from 48.2% to 58.9% in a replay, and a
/// 1 s cap brought it to 47.0% (NOTSOFAR unchanged, 41.9% to 41.8%; `DECISION_LOG.md`).
const WORD_MAX_MS: u64 = 1_000;
/// A word further than this from every speaker segment has a guessed speaker (#91).
const SPEAKER_DOUBT_MS: u64 = 2_000;

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
    /// The engine was unsure of the word (#91).
    uncertain: bool,
}

impl MeetingWord {
    /// Creates a word; an end before the start is raised to the start, and a word longer than
    /// `WORD_MAX_MS` is cut to that length from its start.
    pub fn new(text: impl Into<String>, start_ms: u64, end_ms: u64) -> Self {
        Self {
            text: text.into(),
            start_ms,
            end_ms: end_ms.clamp(start_ms, start_ms.saturating_add(WORD_MAX_MS)),
            uncertain: false,
        }
    }

    /// Marks whether the engine was unsure of the word (#91). Side effects: None.
    pub fn with_uncertain(mut self, uncertain: bool) -> Self {
        self.uncertain = uncertain;
        self
    }

    /// Reports whether the engine was unsure of the word.
    pub fn is_uncertain(&self) -> bool {
        self.uncertain
    }

    /// Returns the word as written.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns when the word starts, in meeting time.
    pub fn start_ms(&self) -> u64 {
        self.start_ms
    }

    /// Returns when the word ends, in meeting time.
    pub fn end_ms(&self) -> u64 {
        self.end_ms
    }

    /// Returns the same word moved by `offset_ms`, never before the meeting's start.
    pub(crate) fn shifted(&self, offset_ms: i64) -> Self {
        let shift = |time_ms: u64| time_ms.saturating_add_signed(offset_ms);
        Self {
            text: self.text.clone(),
            start_ms: shift(self.start_ms),
            end_ms: shift(self.end_ms),
            uncertain: self.uncertain,
        }
    }

    /// Returns the moment that decides who said the word.
    pub(crate) fn midpoint_ms(&self) -> u64 {
        self.start_ms + (self.end_ms - self.start_ms) / 2
    }
}

/// Who a stretch of the meeting belongs to: a named person, an unknown voice, or neither.
pub(crate) type Speaker = (Option<Uuid>, Option<Uuid>);

/// Returns the speaker of the segment containing `time_ms`, else of the nearest segment, and
/// whether it is a guess (#91): that segment's own speaker was uncertain, or none lies within
/// `SPEAKER_DOUBT_MS`.
pub(crate) fn speaker_with_doubt(speakers: &[TranscriptSegment], time_ms: u64) -> (Speaker, bool) {
    let distance = |segment: &TranscriptSegment| {
        if time_ms < segment.start_time_ms() {
            segment.start_time_ms() - time_ms
        } else {
            time_ms.saturating_sub(segment.end_time_ms())
        }
    };
    speakers
        .iter()
        .min_by_key(|segment| distance(segment))
        .map_or(((None, None), false), |segment| {
            (
                (segment.speaker_identity_id(), segment.unknown_cluster_id()),
                segment.confidence().uncertain_speaker || distance(segment) > SPEAKER_DOUBT_MS,
            )
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
    segments_of(
        meeting_id,
        words.iter().map(|word| {
            let (speaker, doubtful) = speaker_with_doubt(speakers, word.midpoint_ms());
            (word, speaker, doubtful)
        }),
    )
}

/// Groups words, each with its speaker, into segments.
///
/// Inputs: The meeting and its words in time order, each with who said it and whether that is a
/// guess.
/// Outputs: Segments of consecutive words with one speaker, split at pauses over 2 s and kept
/// within 20 s; blank words are dropped. Each segment marks its uncertain words, and its speaker
/// as uncertain when any of its words' speaker was a guess (#91); overlap is marked afterwards.
/// Side effects: None.
pub(crate) fn segments_of<'word>(
    meeting_id: &MeetingId,
    words: impl IntoIterator<Item = (&'word MeetingWord, Speaker, bool)>,
) -> Vec<TranscriptSegment> {
    struct Run<'text> {
        speaker: Speaker,
        start_ms: u64,
        end_ms: u64,
        texts: Vec<&'text str>,
        uncertain: Vec<u16>,
        doubtful: bool,
    }
    let mut runs: Vec<Run<'_>> = Vec::new();
    for (word, speaker, doubtful) in words {
        let text = word.text.trim();
        if text.is_empty() {
            continue;
        }
        let joins = runs.last().is_some_and(|run| {
            run.speaker == speaker
                && word.start_ms.saturating_sub(run.end_ms) <= SEGMENT_PAUSE_MS
                && word.end_ms.saturating_sub(run.start_ms) <= SEGMENT_MAX_MS
        });
        if !joins {
            runs.push(Run {
                speaker,
                start_ms: word.start_ms,
                end_ms: word.end_ms,
                texts: Vec::new(),
                uncertain: Vec::new(),
                doubtful: false,
            });
        }
        if let Some(run) = runs.last_mut() {
            if word.uncertain {
                let position = run
                    .texts
                    .iter()
                    .map(|text| text.split_whitespace().count())
                    .sum::<usize>();
                run.uncertain.extend(
                    (position..position + text.split_whitespace().count())
                        .map(|word| u16::try_from(word).unwrap_or(u16::MAX)),
                );
            }
            run.end_ms = run.end_ms.max(word.end_ms);
            run.texts.push(text);
            run.doubtful |= doubtful;
        }
    }
    runs.into_iter()
        .map(|run| {
            let (identity_id, cluster_id) = run.speaker;
            TranscriptSegment::new(
                meeting_id.clone(),
                run.start_ms,
                run.end_ms,
                identity_id,
                run.texts.join(" "),
            )
            .with_unknown_cluster_id(cluster_id)
            .with_confidence(SegmentConfidence {
                overlapping_speech: false,
                uncertain_speaker: run.doubtful,
                uncertain_words: run.uncertain,
            })
        })
        .collect()
}

/// Reads a meeting's overlapping speech for a pass; a storage failure only leaves it unmarked.
///
/// Side effects: Reads the meeting's document; logs a failure without content.
pub(crate) fn overlap_of<Repository: NoteRepository + ?Sized>(
    repository: &Repository,
    meeting_id: &MeetingId,
) -> Vec<OverlapSpan> {
    repository
        .load_overlap_spans(meeting_id)
        .unwrap_or_else(|error| {
            tracing::warn!(
                meeting_id = %meeting_id.as_str(),
                error = %error,
                "Could not read overlapping speech; the new transcript does not mark it"
            );
            Vec::new()
        })
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
/// example recorded before retention was on) are kept as they are, and so are the segments heard
/// on the computer's sound (#108), whose audio is not retained.
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
    let (words, window_count) = transcribe_chunks(
        meeting_id,
        meeting_id.as_str(),
        chunks,
        engine,
        provide_audio,
        device_id,
        cancelled,
    )?;
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
    let (speakers, kept): (Vec<TranscriptSegment>, Vec<TranscriptSegment>) = standing
        .into_iter()
        .partition(|segment| segment.source() == SegmentSource::Microphone && covered(segment));
    let refined = mark_overlapping_segments(
        reattribute_words(meeting_id, &words, &speakers),
        &overlap_of(repository, meeting_id),
    );
    let segment_count = refined.len();
    let mut timeline = kept;
    timeline.extend(refined);
    timeline.sort_by_key(TranscriptSegment::start_time_ms);
    repository.save_transcript_segments(meeting_id, &timeline)?;
    Ok(RefinementOutcome {
        window_count,
        segment_count,
    })
}

/// Re-transcribes speech chunks in long windows.
///
/// Inputs: The meeting the audio belongs to; a name for the windows' audio ids; the chunks in time
/// order; the engine; `provide_audio`; this device; and `cancelled`, polled before each window.
/// Outputs: The words in the chunks' time, and the number of windows, or why it stopped. Each
/// window is prompted with the previous one's text.
/// Side effects: Runs inference.
pub(crate) fn transcribe_chunks<Engine>(
    meeting_id: &MeetingId,
    audio_name: &str,
    chunks: &[RefinementChunk<'_>],
    engine: &Engine,
    provide_audio: &dyn Fn(&RecordingId, Vec<i16>),
    device_id: &DeviceId,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<MeetingWord>, usize), RefinementError>
where
    Engine: TranscriptionEngine + ?Sized,
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
        let recording_id = RecordingId::new(format!("refine-{audio_name}-{window_number}"));
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
            .with_uncertain(is_uncertain_probability(word.probability_permille()))
        }));
        let text = transcript.content().trim();
        previous_text = (!text.is_empty()).then(|| text.to_owned());
    }
    Ok((words, windows.len()))
}
