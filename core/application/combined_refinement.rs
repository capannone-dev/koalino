//! Transcribes one meeting recorded on several devices from all of them (#88).
//!
//! The owner may put the laptop near themselves and a phone near the person they talk to. Each
//! device then hears its own person well and the other badly, as a close-talk microphone does
//! (on AMI, close-talk audio cut word error by 5–10 points). After the meeting, the combining
//! device has every recording's sealed speech (`combined_meetings`). Here:
//!
//! 1. each recording's speech is re-transcribed in long windows, as in the single pass (#87);
//! 2. the clock offset from the fold is refined from words both devices wrote;
//! 3. every word takes its speaker from its own device's final speaker pass, and each speaker of
//!    another device is mapped to the first recording's speaker of the same person (the same named
//!    person, else the one they share the most time with);
//! 4. each person's words are taken from the device that wrote more of them, so in overlapping
//!    speech each device keeps the person near it;
//! 5. a word written for both devices within a second (or spelt a letter apart, for long words),
//!    within three seconds with a neighbouring word the same too, or within eight with two, is
//!    kept once. The copy set inside more of its own sentence stays; then the copy under the
//!    speaker of the words before it, so a turn keeps its last word when the other device started
//!    the next turn early; else the one under the speaker with more words, so a sentence one
//!    device split off stays in its main voice;
//! 6. a word or two set apart mid-sentence, between words of one speaker, go back to that
//!    speaker; a one-word answer after a question stays its own turn; and a speaker change a
//!    word or two off a sentence end moves to it.
//!
//! Before that, a chunk of one recording that lost a sentence its own text had keeps that text
//! (`keep_live_where_the_pass_lost_speech`), as in the single pass.
//!
//! On NOTSOFAR, where every device stands far from everyone, choosing a device per speaker was
//! neutral (38.9% word error either way), so a badly placed device does not make things worse.
//! `combine_sources` is pure; `refine_combined_meeting` drives the engine and the repository.

use std::collections::HashMap;

use crate::{
    application::{
        echo_removal::without_echoes,
        same_meeting::{RecordingText, comparison_key, recognise_same_meeting},
        transcript_refinement::{
            ChunkSpan, MeetingWord, RefinementChunk, RefinementError, RefinementOutcome, Speaker,
            add_missing_review_clips, keep_live_where_the_pass_lost_speech, overlap_of,
            segments_of, speaker_with_doubt, transcribe_chunks,
        },
    },
    domain::{
        models::{DeviceId, MeetingId, RecordingId, SegmentSource, TranscriptSegment},
        transcript_confidence::{mark_overlapping_segments, merge_overlap_spans},
    },
    ports::{ai::TranscriptionEngine, storage::NoteRepository},
};

/// Two devices' copies of a word closer than this are one word.
const DUPLICATE_WINDOW_MS: u64 = 1_000;
/// Two devices' copies of a word up to this far apart are one word when a neighbouring word is
/// the same too: their clocks and word times can disagree by more than a second (#88).
const DUPLICATE_PHRASE_WINDOW_MS: u64 = 3_000;
/// How much further apart than the word itself its confirming neighbour's copies may be.
const NEIGHBOUR_DRIFT_MS: u64 = 500;
/// Two devices' copies of a word up to this far apart are one word when two of the words around
/// them are the same too: the engine can misplace a whole phrase by seconds (#88 run 4).
const DUPLICATE_DRIFTED_WINDOW_MS: u64 = 8_000;
/// Two words this close, the first ending no sentence, belong to one sentence (one device can
/// leave a second or more between words the other device wrote together).
const SENTENCE_FLOW_GAP_MS: u64 = 3_000;
/// Words shorter than this are not matched by spelling alone: short words differ by one letter.
const NEARLY_SAME_MIN_LETTERS: usize = 6;
/// At most this many words set apart mid-sentence go back to the speaker around them.
const STRAY_TURN_MAX_WORDS: usize = 2;
/// ... and only when they last at most this long.
const STRAY_TURN_MAX_MS: u64 = 1_500;
/// At most this many words move across a speaker change to the sentence they belong to...
const TURN_SNAP_MAX_WORDS: usize = 2;
/// ... when they are at most this far from the turn they join.
const TURN_SNAP_GAP_MS: u64 = 1_500;

/// One recording's re-transcribed words and its speakers.
pub struct SourceWords<'speakers> {
    /// The words, in this recording's time.
    pub words: Vec<MeetingWord>,
    /// Where this recording sits on the first recording's timeline (0 for the first itself).
    pub offset_ms: i64,
    /// This recording's final speakers, in its own time.
    pub speakers: &'speakers [TranscriptSegment],
}

/// One word placed on the first recording's timeline.
struct PlacedWord {
    word: MeetingWord,
    speaker: Speaker,
    source: usize,
    /// The word's speaker is a guess (#91).
    doubtful: bool,
}

/// Builds the meeting's segments from several recordings' words.
///
/// Inputs: The meeting (the first recording), its words and speakers, and the other recordings'.
/// Outputs: Segments in the first recording's time (see the module header). Side effects: None.
pub fn combine_sources(
    meeting_id: &MeetingId,
    root: &SourceWords<'_>,
    members: &[SourceWords<'_>],
) -> Vec<TranscriptSegment> {
    let mut placed: Vec<PlacedWord> = Vec::new();
    for (source, words) in std::iter::once(root).chain(members).enumerate() {
        let speaker_map = if source == 0 {
            HashMap::new()
        } else {
            map_speakers(words, root.speakers)
        };
        for word in &words.words {
            let (own, doubtful) = speaker_with_doubt(words.speakers, word.midpoint_ms());
            let speaker = if source == 0 {
                own
            } else {
                speaker_map.get(&own).copied().unwrap_or(own)
            };
            placed.push(PlacedWord {
                word: word.shifted(words.offset_ms),
                speaker,
                source,
                doubtful,
            });
        }
    }
    let mut placed_keys: Vec<SourceEntry> = placed
        .iter()
        .map(|word| SourceEntry {
            source: word.source,
            start_ms: word.word.start_ms(),
            end_ms: word.word.end_ms(),
            key: comparison_key(word.word.text()),
            ends_sentence: ends_sentence(word.word.text()),
        })
        .collect();
    placed_keys.sort_by_key(|entry| (entry.source, entry.start_ms));
    let mut counts: HashMap<Speaker, Vec<usize>> = HashMap::new();
    for word in &placed {
        let per_source = counts
            .entry(word.speaker)
            .or_insert_with(|| vec![0; members.len() + 1]);
        per_source[word.source] += 1;
    }
    // The device with the most words for each person; the first recording wins a tie.
    let chosen: HashMap<Speaker, usize> = counts
        .into_iter()
        .map(|(speaker, per_source)| {
            let best = per_source
                .iter()
                .enumerate()
                .fold(0, |best, (source, count)| {
                    if *count > per_source[best] {
                        source
                    } else {
                        best
                    }
                });
            (speaker, best)
        })
        .collect();
    let mut kept: Vec<PlacedWord> = placed
        .into_iter()
        .filter(|word| chosen.get(&word.speaker) == Some(&word.source))
        .collect();
    kept.sort_by_key(|word| (word.word.start_ms(), word.source));
    // Each source's words in time order, to compare a word's neighbours across devices.
    let mut by_source: Vec<Vec<SourceEntry>> = vec![Vec::new(); members.len() + 1];
    for word in &placed_keys {
        by_source[word.source].push(word.clone());
    }
    let position_of = |word: &PlacedWord| {
        by_source[word.source]
            .iter()
            .position(|entry| entry.start_ms == word.word.start_ms())
    };
    // The same words around both copies: a neighbour at the same distance (the two devices' words
    // drift apart together) confirms a copy within `DUPLICATE_PHRASE_WINDOW_MS`, so a fragment
    // the far device caught does not; two neighbours confirm one up to
    // `DUPLICATE_DRIFTED_WINDOW_MS`, where the engine misplaced a whole phrase.
    let neighbours_match = |first: &PlacedWord, second: &PlacedWord| {
        let around = |word: &PlacedWord| -> [Option<&SourceEntry>; 4] {
            let sequence = &by_source[word.source];
            position_of(word).map_or([None; 4], |index| {
                let at = |step: isize| {
                    index
                        .checked_add_signed(step)
                        .and_then(|other| sequence.get(other))
                };
                [at(-2), at(-1), at(1), at(2)]
            })
        };
        let shift = |earlier: u64, later: u64| i128::from(later) - i128::from(earlier);
        let word_shift = shift(first.word.start_ms(), second.word.start_ms());
        let pairs: Vec<(&SourceEntry, &SourceEntry)> = around(first)
            .into_iter()
            .zip(around(second))
            .filter_map(|pair| match pair {
                (Some(one), Some(other)) if one.key == other.key => Some((one, other)),
                _ => None,
            })
            .collect();
        let near = pairs.iter().any(|(one, other)| {
            (shift(one.start_ms, other.start_ms) - word_shift).abs()
                <= i128::from(NEIGHBOUR_DRIFT_MS)
        });
        let apart = word_shift.unsigned_abs();
        (near && apart <= u128::from(DUPLICATE_PHRASE_WINDOW_MS)) || pairs.len() >= 2
    };
    // How many of a copy's neighbours on its own device run on into it: 0 to 2.
    let flow = |word: &PlacedWord| {
        let sequence = &by_source[word.source];
        position_of(word).map_or(0, |index| {
            let joined = |first: &SourceEntry, second: &SourceEntry| {
                !first.ends_sentence
                    && second.start_ms.saturating_sub(first.end_ms) <= SENTENCE_FLOW_GAP_MS
            };
            let before = index
                .checked_sub(1)
                .and_then(|earlier| sequence.get(earlier))
                .is_some_and(|earlier| joined(earlier, &sequence[index]));
            let after = sequence
                .get(index + 1)
                .is_some_and(|later| joined(&sequence[index], later));
            usize::from(before) + usize::from(after)
        })
    };
    // How many words each speaker keeps: a copy goes to the speaker carrying more of the meeting.
    let mut weight: HashMap<Speaker, usize> = HashMap::new();
    for word in &kept {
        *weight.entry(word.speaker).or_default() += 1;
    }
    let mut written: Vec<&PlacedWord> = Vec::with_capacity(kept.len());
    for word in &kept {
        let key = comparison_key(word.word.text());
        let copy = written
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_, earlier)| {
                word.word.start_ms().saturating_sub(earlier.word.start_ms())
                    <= DUPLICATE_DRIFTED_WINDOW_MS
            })
            .find(|(_, earlier)| {
                let apart = word.word.start_ms().saturating_sub(earlier.word.start_ms());
                let earlier_key = comparison_key(earlier.word.text());
                earlier.source != word.source
                    && ((earlier_key == key
                        && (apart <= DUPLICATE_WINDOW_MS || neighbours_match(earlier, word)))
                        || (apart <= DUPLICATE_WINDOW_MS && nearly_same(&earlier_key, &key)))
            })
            .map(|(index, _)| index);
        match copy {
            None => written.push(word),
            Some(index) => {
                // The copy set inside more of its own sentence stays; then the copy whose speaker
                // continues the words before it (a turn keeps its last word); then the speaker
                // with more words, so a sentence one device split off stays in its main voice.
                let before = index
                    .checked_sub(1)
                    .and_then(|earlier| written.get(earlier))
                    .map(|earlier| earlier.speaker);
                let weight_of = |speaker: &Speaker| weight.get(speaker).copied().unwrap_or(0);
                let (new_flow, old_flow) = (flow(word), flow(written[index]));
                let takes_over = if new_flow != old_flow {
                    new_flow > old_flow
                } else if before == Some(written[index].speaker) {
                    false
                } else if before == Some(word.speaker) {
                    true
                } else {
                    weight_of(&word.speaker) > weight_of(&written[index].speaker)
                };
                if takes_over {
                    written[index] = word;
                }
            }
        }
    }
    written.sort_by_key(|word| (word.word.start_ms(), word.source));
    let mut items: Vec<(&MeetingWord, Speaker, bool, usize)> = written
        .into_iter()
        .map(|word| (&word.word, word.speaker, word.doubtful, word.source))
        .collect();
    keep_sentences_whole(&mut items);
    segments_of(
        meeting_id,
        items
            .into_iter()
            .map(|(word, speaker, doubtful, _)| (word, speaker, doubtful)),
    )
}

/// One word of one source, as the copy checks compare it.
#[derive(Clone)]
struct SourceEntry {
    source: usize,
    start_ms: u64,
    end_ms: u64,
    key: String,
    /// The word as written ends with `.`, `?` or `!`.
    ends_sentence: bool,
}

/// Reports whether a word ends a sentence. Side effects: None.
fn ends_sentence(text: &str) -> bool {
    text.trim_end_matches(['"', '\'', ')', '»'])
        .ends_with(['.', '?', '!', '…'])
}

/// Reports whether two different comparison keys are one word spelt a little differently (one
/// letter changed, added or dropped, apostrophes aside), for words of six letters or more.
///
/// Inputs: Two comparison keys. Outputs: true for such a pair. Side effects: None.
fn nearly_same(first: &str, second: &str) -> bool {
    let letters = |key: &str| -> Vec<char> { key.chars().filter(|c| *c != '\'').collect() };
    let (first, second) = (letters(first), letters(second));
    let (shorter, longer) = if first.len() <= second.len() {
        (&first, &second)
    } else {
        (&second, &first)
    };
    if shorter.len() < NEARLY_SAME_MIN_LETTERS || longer.len() - shorter.len() > 1 {
        return false;
    }
    let common_start = shorter
        .iter()
        .zip(longer.iter())
        .take_while(|(one, other)| one == other)
        .count();
    let common_end = shorter
        .iter()
        .rev()
        .zip(longer.iter().rev())
        .take_while(|(one, other)| one == other)
        .count();
    // One edit: what is left between the common start and end is at most one letter each.
    common_start + common_end + 1 >= longer.len()
}

/// Gives a word or two the speaker pass set apart in the middle of a sentence to the speaker
/// around them: the words before end no sentence, the same speaker talks on both sides, the
/// stray words last under `STRAY_TURN_MAX_MS`, and all come from one device (words of two
/// devices interleaving are two people talking at once). A one-word answer after a question
/// stays its own.
///
/// Inputs: The words in time order with their speakers, doubt and source. Side effects: Changes
/// those speakers.
fn keep_sentences_whole(items: &mut [(&MeetingWord, Speaker, bool, usize)]) {
    let mut start = 0;
    while start < items.len() {
        let speaker = items[start].1;
        let end = items[start..]
            .iter()
            .position(|item| item.1 != speaker)
            .map_or(items.len(), |offset| start + offset);
        if start > 0 && end < items.len() {
            let (before, after) = (items[start - 1], items[end]);
            let lasts = items[end - 1]
                .0
                .end_ms()
                .saturating_sub(items[start].0.start_ms());
            let one_device = items[start - 1..=end].iter().all(|item| item.3 == before.3);
            if before.1 == after.1
                && one_device
                && end - start <= STRAY_TURN_MAX_WORDS
                && lasts <= STRAY_TURN_MAX_MS
                && !ends_sentence(before.0.text())
            {
                for item in &mut items[start..end] {
                    item.1 = before.1;
                }
            }
        }
        start = end;
    }
    snap_turn_changes_to_sentences(items);
}

/// Moves each speaker change to the sentence end next to it: up to two words opening a sentence
/// at the end of a turn go to the next speaker, and up to two words ending a sentence at the
/// start of a turn go back to the previous one, when they are close to the turn they join. The
/// speaker pass often changes speaker a word late or early (#88 run 4).
///
/// Inputs: The words in time order with their speakers. Side effects: Changes those speakers.
fn snap_turn_changes_to_sentences(items: &mut [(&MeetingWord, Speaker, bool, usize)]) {
    for change in 1..items.len() {
        let (previous, next) = (items[change - 1].1, items[change].1);
        if previous == next {
            continue;
        }
        let joins = |earlier: &MeetingWord, later: &MeetingWord| {
            later.start_ms().saturating_sub(earlier.end_ms()) <= TURN_SNAP_GAP_MS
        };
        // A sentence opening at the end of the previous turn.
        let opening = (1..=TURN_SNAP_MAX_WORDS).find(|count| {
            change > *count
                && ends_sentence(items[change - count - 1].0.text())
                && items[change - count..change]
                    .iter()
                    .all(|item| item.1 == previous && !ends_sentence(item.0.text()))
        });
        if let Some(count) = opening
            && joins(items[change - 1].0, items[change].0)
        {
            for item in &mut items[change - count..change] {
                item.1 = next;
            }
            continue;
        }
        // A sentence ending at the start of the next turn.
        let closing = (1..=TURN_SNAP_MAX_WORDS).find(|count| {
            change + count <= items.len()
                && ends_sentence(items[change + count - 1].0.text())
                && items[change..change + count - 1]
                    .iter()
                    .all(|item| !ends_sentence(item.0.text()))
                && items[change..change + count]
                    .iter()
                    .all(|item| item.1 == next)
        });
        if let Some(count) = closing
            && !ends_sentence(items[change - 1].0.text())
            && joins(items[change - 1].0, items[change].0)
        {
            for item in &mut items[change..change + count] {
                item.1 = previous;
            }
        }
    }
}

/// Maps each speaker of another recording to the first recording's speaker of the same person.
///
/// Outputs: For each of the recording's speakers, the same named person when the first recording
/// has them, else the first recording's speaker it shares the most time with; speakers sharing
/// no time keep their own. Side effects: None.
fn map_speakers(
    member: &SourceWords<'_>,
    root_speakers: &[TranscriptSegment],
) -> HashMap<Speaker, Speaker> {
    let speaker_of =
        |segment: &TranscriptSegment| (segment.speaker_identity_id(), segment.unknown_cluster_id());
    let mut shared: HashMap<Speaker, HashMap<Speaker, u64>> = HashMap::new();
    for segment in member.speakers {
        let start = segment
            .start_time_ms()
            .saturating_add_signed(member.offset_ms);
        let end = segment
            .end_time_ms()
            .saturating_add_signed(member.offset_ms);
        let by_root = shared.entry(speaker_of(segment)).or_default();
        for root_segment in root_speakers {
            let overlap = end
                .min(root_segment.end_time_ms())
                .saturating_sub(start.max(root_segment.start_time_ms()));
            if overlap > 0 {
                *by_root.entry(speaker_of(root_segment)).or_default() += overlap;
            }
        }
    }
    let named_in_root: Vec<Speaker> = root_speakers.iter().map(speaker_of).collect();
    shared
        .into_iter()
        .filter_map(|(speaker, by_root)| {
            let same_person = speaker.0.and_then(|identity| {
                named_in_root
                    .iter()
                    .find(|root| root.0 == Some(identity))
                    .copied()
            });
            same_person
                .or_else(|| {
                    by_root
                        .into_iter()
                        .max_by_key(|(_, overlap)| *overlap)
                        .map(|(root, _)| root)
                })
                .map(|root| (speaker, root))
        })
        .collect()
}

/// Refines where another recording sits on the first one's timeline from words both wrote.
///
/// Inputs: The first recording's words, the other's (each in its own time), and the fold's
/// offset. Outputs: The offset from the words both share, or the fold's when they share too
/// little. Side effects: None.
pub fn refine_offset(root: &[MeetingWord], member: &[MeetingWord], fold_offset_ms: i64) -> i64 {
    recognise_same_meeting(
        &RecordingText::from_words(0, root.iter().map(|word| (word.start_ms(), word.text()))),
        &RecordingText::from_words(
            fold_offset_ms,
            member.iter().map(|word| (word.start_ms(), word.text())),
        ),
    )
    .map_or(fold_offset_ms, |same| same.offset_ms())
}

/// Gathers the segments every recording heard on the computer's sound (#108), which the pass does
/// not re-transcribe.
///
/// Inputs: The first recording's meeting; each recording's segments with its offset on the first
/// recording's timeline. Outputs: Those segments moved onto that timeline and into that meeting,
/// in time order; an unknown-speaker link of another meeting is dropped, as its clusters stay
/// there. Side effects: None.
pub(crate) fn computer_sound_on_first(
    root_meeting_id: &MeetingId,
    recordings: &[(&[TranscriptSegment], i64)],
) -> Vec<TranscriptSegment> {
    let mut heard: Vec<TranscriptSegment> = recordings
        .iter()
        .flat_map(|(segments, offset_ms)| {
            segments
                .iter()
                .filter(|segment| segment.source() == SegmentSource::ComputerSound)
                .map(move |segment| {
                    let same_meeting = segment.meeting_id() == root_meeting_id;
                    TranscriptSegment::new(
                        root_meeting_id.clone(),
                        segment.start_time_ms().saturating_add_signed(*offset_ms),
                        segment.end_time_ms().saturating_add_signed(*offset_ms),
                        segment.speaker_identity_id(),
                        segment.text(),
                    )
                    .with_unknown_cluster_id(segment.unknown_cluster_id().filter(|_| same_meeting))
                    .with_confidence(segment.confidence().clone())
                    .with_source(SegmentSource::ComputerSound)
                })
        })
        .collect();
    heard.sort_by_key(TranscriptSegment::start_time_ms);
    heard
}

/// One recording handed to `refine_combined_meeting`.
pub struct CombinedRecording<'chunks> {
    /// The recording's meeting.
    pub meeting_id: MeetingId,
    /// Its retained chunks in its own time.
    pub chunks: Vec<RefinementChunk<'chunks>>,
    /// Where it sits on the first recording's timeline, as folded (0 for the first).
    pub offset_ms: i64,
}

/// Re-transcribes a meeting from several devices' speech and replaces the first recording's text.
///
/// Inputs: The first recording and the others; the engine; `provide_audio`; the repository;
/// this device; and `cancelled`, polled before each window.
/// Outputs: The outcome, or why nothing changed. Segments of the first recording whose middle
/// lies outside every recording's speech are kept as they are, and so is every recording's
/// computer sound (#108), moved onto the first recording's timeline.
/// Side effects: Runs inference and rewrites the first recording's segments (and its synced
/// document). The other recordings' meetings are left as they are, folded and hidden.
pub fn refine_combined_meeting<Engine, Repository>(
    root: &CombinedRecording<'_>,
    members: &[CombinedRecording<'_>],
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
    let mut window_count = 0;
    let mut transcribed: Vec<(Vec<MeetingWord>, Vec<TranscriptSegment>, i64)> = Vec::new();
    let mut root_words: Option<Vec<MeetingWord>> = None;
    let mut computer_sound: Vec<(Vec<TranscriptSegment>, i64)> = Vec::new();
    for recording in std::iter::once(root).chain(members) {
        let (words, windows) = transcribe_chunks(
            &root.meeting_id,
            recording.meeting_id.as_str(),
            &recording.chunks,
            engine,
            provide_audio,
            device_id,
            cancelled,
        )?;
        window_count += windows;
        let (heard, speakers): (Vec<TranscriptSegment>, Vec<TranscriptSegment>) = repository
            .load_transcript_segments(&recording.meeting_id)?
            .into_iter()
            .partition(|segment| segment.source() == SegmentSource::ComputerSound);
        // A chunk that lost a sentence this recording's own text had keeps that text.
        let spans: Vec<ChunkSpan> = recording.chunks.iter().map(|chunk| chunk.span).collect();
        let (words, kept_live) = keep_live_where_the_pass_lost_speech(words, &spans, &speakers);
        if kept_live > 0 {
            tracing::info!(meeting_id = %recording.meeting_id.as_str(), chunk_count = kept_live, "The pass lost speech in some chunks; their live text stands");
        }
        let offset_ms = match &root_words {
            None => 0,
            Some(root_words) => refine_offset(root_words, &words, recording.offset_ms),
        };
        if root_words.is_none() {
            root_words = Some(words.clone());
        }
        computer_sound.push((heard, offset_ms));
        transcribed.push((words, speakers, offset_ms));
    }
    if cancelled() {
        return Err(RefinementError::Cancelled);
    }
    let mut spans: Vec<ChunkSpan> = Vec::new();
    for (recording, (_, _, offset_ms)) in std::iter::once(root).chain(members).zip(&transcribed) {
        spans.extend(recording.chunks.iter().map(|chunk| ChunkSpan {
            start_ms: chunk.span.start_ms.saturating_add_signed(*offset_ms),
            end_ms: chunk.span.end_ms.saturating_add_signed(*offset_ms),
        }));
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
    let mut sources = transcribed
        .iter()
        .map(|(words, speakers, offset_ms)| SourceWords {
            words: words.clone(),
            offset_ms: *offset_ms,
            speakers,
        });
    let Some(root_source) = sources.next() else {
        return Ok(RefinementOutcome {
            window_count,
            segment_count: 0,
        });
    };
    let member_sources: Vec<SourceWords<'_>> = sources.collect();
    let mut kept: Vec<TranscriptSegment> = root_source
        .speakers
        .iter()
        .filter(|segment| !covered(segment))
        .cloned()
        .collect();
    let borrowed: Vec<(&[TranscriptSegment], i64)> = computer_sound
        .iter()
        .map(|(segments, offset_ms)| (segments.as_slice(), *offset_ms))
        .collect();
    kept.extend(computer_sound_on_first(&root.meeting_id, &borrowed));
    // Overlap any recording heard, on the first recording's timeline (#90).
    let mut overlap = Vec::new();
    for (recording, (_, _, offset_ms)) in std::iter::once(root).chain(members).zip(&transcribed) {
        overlap.extend(
            overlap_of(repository, &recording.meeting_id)
                .into_iter()
                .map(|span| span.shifted(*offset_ms)),
        );
    }
    let combined = mark_overlapping_segments(
        combine_sources(&root.meeting_id, &root_source, &member_sources),
        &merge_overlap_spans(overlap),
    );
    let segment_count = combined.len();
    let mut timeline = kept;
    timeline.extend(combined);
    timeline.sort_by_key(TranscriptSegment::start_time_ms);
    // Any recording that heard echo flags the meeting; the audio is gone, so it stands for
    // every window (#108).
    let mut echo_heard = false;
    for recording in std::iter::once(root).chain(members) {
        echo_heard |= repository.load_echo_heard(&recording.meeting_id)?;
    }
    let (timeline, dropped) = without_echoes(timeline, echo_heard, &|_| true);
    let segment_count = segment_count.saturating_sub(dropped);
    repository.save_transcript_segments(&root.meeting_id, &timeline)?;
    // Voices only the final passes found get a clip, cut from whichever recording holds them.
    let mut shifted_chunks = Vec::new();
    for (recording, (_, _, offset_ms)) in std::iter::once(root).chain(members).zip(&transcribed) {
        shifted_chunks.extend(recording.chunks.iter().map(|chunk| RefinementChunk {
            span: ChunkSpan {
                start_ms: chunk.span.start_ms.saturating_add_signed(*offset_ms),
                end_ms: chunk.span.end_ms.saturating_add_signed(*offset_ms),
            },
            language: chunk.language,
            samples: chunk.samples,
        }));
    }
    let recording_meetings: Vec<&MeetingId> = std::iter::once(root)
        .chain(members)
        .map(|recording| &recording.meeting_id)
        .collect();
    add_missing_review_clips(
        &root.meeting_id,
        &recording_meetings,
        &shifted_chunks,
        &timeline,
        repository,
    );
    Ok(RefinementOutcome {
        window_count,
        segment_count,
    })
}
