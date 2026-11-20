//! Re-transcribes a finished meeting in long windows and keeps its speakers (#87).
//!
//! Live transcription cuts speech into short chunks (about 11 s), so Whisper hears little context
//! and words at the cuts suffer. After the meeting, the speech chunks are joined into windows of up
//! to 30 s, the length Whisper was trained on, and transcribed again, possibly with a larger model.
//! Each new word takes the speaker the final speaker pass gave that moment, so names and unknown
//! voices are unchanged and only the text improves. Planning and attribution are pure;
//! `refine_meeting` drives an engine and a repository through their ports. While it holds the
//! speech, it also cuts a review clip for each unknown voice that has none (#116), so the owner
//! can listen before naming it.

use uuid::Uuid;

use crate::{
    application::echo_removal::without_echoes,
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, SegmentSource, TranscriptSegment,
            TranscriptionContext, UnassignedVoiceClip,
        },
        pcm_wav::wav_from_pcm16_le,
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

/// Fewest words the live transcript must have in a chunk before the pass's text there is judged:
/// a few words may be a live hallucination the pass rightly dropped.
const MIN_LIVE_WORDS_JUDGED: usize = 6;
/// The pass must write at least this share (per mille) of a judged chunk's live words.
const MIN_LIVE_WORDS_KEPT_PERMILLE: usize = 500;

/// Keeps the live text of a chunk whose pass lost speech the live transcript had.
///
/// Whisper sometimes skips a sentence inside a long window and writes the next one over it, so
/// the sentence is lost and the next one appears twice (#88: seen on a real two-device test).
/// A chunk is judged when its live text has at least `MIN_LIVE_WORDS_JUDGED` words; if the pass's
/// words there (by midpoint) contain fewer than half of them, the pass lost speech, and that chunk
/// takes the live words instead (spread evenly over each live segment), but only when the pass
/// wrote under half as many words there or mostly wrote a neighbouring chunk's words; a pass
/// with its own full text there stands, since then the live text is the wrong one (a live chunk
/// may even be in another language). The words Whisper misplaced go with the chunk. Words are
/// compared case- and punctuation-free.
///
/// Inputs: The pass's words and the chunks, both in the same time; the standing microphone
/// segments in that time. Outputs: The words to use, in time order, and how many chunks kept the
/// live text. Side effects: None.
pub(crate) fn keep_live_where_the_pass_lost_speech(
    words: Vec<MeetingWord>,
    spans: &[ChunkSpan],
    standing: &[TranscriptSegment],
) -> (Vec<MeetingWord>, usize) {
    use crate::application::same_meeting::comparison_key;
    let inside =
        |span: &ChunkSpan, time_ms: u64| span.start_ms <= time_ms && time_ms <= span.end_ms;
    let live_segments_in = |span: &ChunkSpan| -> Vec<&TranscriptSegment> {
        standing
            .iter()
            .filter(|segment| {
                segment.source() == SegmentSource::Microphone
                    && inside(span, segment_middle(segment))
            })
            .collect()
    };
    let pass_keys_in = |span: &ChunkSpan| -> Vec<String> {
        words
            .iter()
            .filter(|word| inside(span, word.midpoint_ms()))
            .map(|word| comparison_key(word.text()))
            .filter(|key| !key.is_empty())
            .collect()
    };
    // Share (per mille) of `keys` also found in `pool`, each pool word used once.
    let found_permille = |keys: &[String], pool: Vec<String>| -> usize {
        let mut pool = pool;
        let mut found = 0;
        for key in keys {
            if let Some(position) = pool.iter().position(|other| other == key) {
                pool.swap_remove(position);
                found += 1;
            }
        }
        found * 1_000 / keys.len().max(1)
    };
    let live_keys_in = |span: &ChunkSpan| -> Vec<String> {
        live_segments_in(span)
            .into_iter()
            .flat_map(|segment| {
                segment
                    .text()
                    .split_whitespace()
                    .map(comparison_key)
                    .collect::<Vec<_>>()
            })
            .filter(|key| !key.is_empty())
            .collect()
    };
    let lost_speech = |index: usize, span: &ChunkSpan| {
        let live_keys = live_keys_in(span);
        if live_keys.len() < MIN_LIVE_WORDS_JUDGED {
            return false;
        }
        let pass_keys = pass_keys_in(span);
        if found_permille(&live_keys, pass_keys.clone()) >= MIN_LIVE_WORDS_KEPT_PERMILLE {
            return false;
        }
        // The pass disagrees with the live text. It lost speech when it wrote little here, or
        // wrote a neighbouring chunk's words (a sentence written over the skipped one, matching
        // the neighbour's pass or live text);
        // otherwise its own text stands (the live text may be wrong, even in another language).
        if pass_keys.len() * 2 < live_keys.len() {
            return true;
        }
        let neighbours: Vec<&ChunkSpan> = [index.checked_sub(1), index.checked_add(1)]
            .into_iter()
            .flatten()
            .filter_map(|neighbour| spans.get(neighbour))
            .collect();
        let neighbours_pass: Vec<String> = neighbours
            .iter()
            .flat_map(|neighbour| pass_keys_in(neighbour))
            .collect();
        let neighbours_live: Vec<String> = neighbours
            .iter()
            .flat_map(|neighbour| live_keys_in(neighbour))
            .collect();
        found_permille(&pass_keys, neighbours_pass) >= MIN_LIVE_WORDS_KEPT_PERMILLE
            || found_permille(&pass_keys, neighbours_live) >= MIN_LIVE_WORDS_KEPT_PERMILLE
    };
    // Only the chunk that lost speech: the words Whisper misplaced there belong to it, and the
    // rest of its window is the better text (a whole-window fallback measured worse on #88).
    let replaced_spans: Vec<&ChunkSpan> = spans
        .iter()
        .enumerate()
        .filter(|(index, span)| lost_speech(*index, span))
        .map(|(_, span)| span)
        .collect();
    if replaced_spans.is_empty() {
        return (words, 0);
    }
    let in_replaced = |time_ms: u64| replaced_spans.iter().any(|span| inside(span, time_ms));
    let mut kept: Vec<MeetingWord> = words
        .into_iter()
        .filter(|word| !in_replaced(word.midpoint_ms()))
        .collect();
    for span in &replaced_spans {
        for segment in live_segments_in(span) {
            kept.extend(spread_words(segment));
        }
    }
    kept.sort_by_key(MeetingWord::start_ms);
    let replaced_chunks = replaced_spans.len();
    (kept, replaced_chunks)
}

/// The middle of a segment's time.
fn segment_middle(segment: &TranscriptSegment) -> u64 {
    segment.start_time_ms()
        + segment
            .end_time_ms()
            .saturating_sub(segment.start_time_ms())
            / 2
}

/// A segment's words spread evenly over its time.
fn spread_words(segment: &TranscriptSegment) -> Vec<MeetingWord> {
    let texts: Vec<&str> = segment.text().split_whitespace().collect();
    let start = segment.start_time_ms();
    let length = segment.end_time_ms().saturating_sub(start);
    let count = texts.len().max(1) as u64;
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let index = index as u64;
            MeetingWord::new(
                *text,
                start + length * index / count,
                start + length * (index + 1) / count,
            )
        })
        .collect()
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
/// Once echo was heard in the meeting, the new microphone text that only repeats the computer's
/// sound is dropped (#108). A window whose new text lost a sentence the live text had keeps the
/// live text in that chunk (`keep_live_where_the_pass_lost_speech`).
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
    let (words, kept_live) = keep_live_where_the_pass_lost_speech(words, &spans, &standing);
    if kept_live > 0 {
        tracing::info!(meeting_id = %meeting_id.as_str(), chunk_count = kept_live, "The pass lost speech in some chunks; their live text stands");
    }
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
    // The audio is gone, so the meeting's flag stands for every window (#108).
    let (timeline, dropped) =
        without_echoes(timeline, repository.load_echo_heard(meeting_id)?, &|_| true);
    let segment_count = segment_count.saturating_sub(dropped);
    repository.save_transcript_segments(meeting_id, &timeline)?;
    add_missing_review_clips(meeting_id, &[meeting_id], chunks, &timeline, repository);
    Ok(RefinementOutcome {
        window_count,
        segment_count,
    })
}

/// A review clip cut from the retained speech is at least this long...
const MIN_BACKFILLED_CLIP_MS: u64 = 1_000;
/// ...and at most this long.
const MAX_BACKFILLED_CLIP_MS: u64 = 15_000;
/// Samples per millisecond at 16 kHz.
const SAMPLES_PER_MS: u64 = 16;

/// Cuts a review clip for each unknown voice of the meeting that has none (#116).
///
/// The final speaker pass can find a voice the live pass never clustered; such a voice has no
/// clip, and the owner could only name it from its words. The voice's longest microphone segment
/// that lies inside one retained chunk gives the clip, up to 15 s of it; a voice with under 1 s
/// there gets none, as live clips do.
///
/// Inputs: The meeting the clips belong to; every meeting whose clips count as a voice's clip
/// (the recordings of a combined meeting, #88); the retained chunks on the timeline's clock; and
/// the final timeline.
/// Outputs: None. Side effects: Saves new clips (raw audio until the voice is named, as live
/// clips); a storage failure is logged and changes nothing else.
pub(crate) fn add_missing_review_clips<Repository>(
    meeting_id: &MeetingId,
    clip_meetings: &[&MeetingId],
    chunks: &[RefinementChunk<'_>],
    timeline: &[TranscriptSegment],
    repository: &Repository,
) where
    Repository: NoteRepository + ?Sized,
{
    let mut clipped: Vec<Uuid> = Vec::new();
    for clip_meeting in clip_meetings {
        match repository.get_unassigned_clips(clip_meeting) {
            Ok(clips) => clipped.extend(clips.iter().filter_map(UnassignedVoiceClip::cluster_id)),
            Err(error) => {
                tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "Review clips could not be read; none were added");
                return;
            }
        }
    }
    let mut longest: Vec<(Uuid, &TranscriptSegment)> = Vec::new();
    for segment in timeline
        .iter()
        .filter(|segment| segment.source() == SegmentSource::Microphone)
        .filter(|segment| segment.speaker_identity_id().is_none())
    {
        let Some(cluster_id) = segment.unknown_cluster_id() else {
            continue;
        };
        if clipped.contains(&cluster_id) || !inside_one_chunk(chunks, segment) {
            continue;
        }
        let length = |segment: &TranscriptSegment| {
            segment
                .end_time_ms()
                .saturating_sub(segment.start_time_ms())
        };
        match longest.iter_mut().find(|(known, _)| *known == cluster_id) {
            Some((_, best)) if length(segment) > length(best) => *best = segment,
            Some(_) => {}
            None => longest.push((cluster_id, segment)),
        }
    }
    for (cluster_id, segment) in longest {
        let start_ms = segment.start_time_ms();
        let end_ms = segment.end_time_ms().min(start_ms + MAX_BACKFILLED_CLIP_MS);
        if end_ms.saturating_sub(start_ms) < MIN_BACKFILLED_CLIP_MS {
            continue;
        }
        let Some(samples) = samples_between(chunks, start_ms, end_ms) else {
            continue;
        };
        let pcm: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let clip = UnassignedVoiceClip::new(
            Uuid::new_v4(),
            meeting_id.clone(),
            0,
            end_ms - start_ms,
            wav_from_pcm16_le(&pcm, 16_000),
        )
        .with_cluster_id(Some(cluster_id));
        if let Err(error) = repository.save_unassigned_clip(clip) {
            tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "A review clip could not be cut from the retained speech");
        }
    }
}

/// Whether a segment lies inside one retained chunk.
fn inside_one_chunk(chunks: &[RefinementChunk<'_>], segment: &TranscriptSegment) -> bool {
    chunks.iter().any(|chunk| {
        chunk.span.start_ms <= segment.start_time_ms() && segment.end_time_ms() <= chunk.span.end_ms
    })
}

/// The samples of one chunk between two meeting times, or `None` when no chunk holds both.
fn samples_between<'samples>(
    chunks: &[RefinementChunk<'samples>],
    start_ms: u64,
    end_ms: u64,
) -> Option<&'samples [i16]> {
    let chunk = chunks
        .iter()
        .find(|chunk| chunk.span.start_ms <= start_ms && end_ms <= chunk.span.end_ms)?;
    let first = usize::try_from((start_ms - chunk.span.start_ms) * SAMPLES_PER_MS).ok()?;
    let last = usize::try_from((end_ms - chunk.span.start_ms) * SAMPLES_PER_MS).ok()?;
    chunk.samples.get(first..last.min(chunk.samples.len()))
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
