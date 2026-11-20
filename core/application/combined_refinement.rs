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
//! 5. a word written for both devices within a second, or within three seconds with a
//!    neighbouring word the same too, is kept once, under the speaker with more words: when one
//!    device's speaker pass gave part of a sentence to a speaker of its own, the sentence stays
//!    whole in its main voice.
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
    let mut placed_keys: Vec<(usize, u64, String)> = placed
        .iter()
        .map(|word| {
            (
                word.source,
                word.word.start_ms(),
                comparison_key(word.word.text()),
            )
        })
        .collect();
    placed_keys.sort_by_key(|(source, start, _)| (*source, *start));
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
    let mut by_source: Vec<Vec<(u64, String)>> = vec![Vec::new(); members.len() + 1];
    for word in &placed_keys {
        by_source[word.0].push((word.1, word.2.clone()));
    }
    // A neighbour confirms a copy when it is the same word at the same distance (the two devices'
    // words drift apart together), so a fragment the far device caught does not.
    let neighbours_match = |first: &PlacedWord, second: &PlacedWord| {
        let around = |word: &PlacedWord| -> [Option<&(u64, String)>; 2] {
            let sequence = &by_source[word.source];
            sequence
                .iter()
                .position(|(start, _)| *start == word.word.start_ms())
                .map_or([None, None], |index| {
                    [
                        index.checked_sub(1).and_then(|before| sequence.get(before)),
                        sequence.get(index + 1),
                    ]
                })
        };
        let shift = |earlier: u64, later: u64| i128::from(later) - i128::from(earlier);
        let word_shift = shift(first.word.start_ms(), second.word.start_ms());
        around(first)
            .into_iter()
            .zip(around(second))
            .any(|pair| match pair {
                (Some((first_start, first_key)), Some((second_start, second_key))) => {
                    first_key == second_key
                        && (shift(*first_start, *second_start) - word_shift).abs()
                            <= i128::from(NEIGHBOUR_DRIFT_MS)
                }
                _ => false,
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
                    <= DUPLICATE_PHRASE_WINDOW_MS
            })
            .find(|(_, earlier)| {
                earlier.source != word.source
                    && comparison_key(earlier.word.text()) == key
                    && (word.word.start_ms().saturating_sub(earlier.word.start_ms())
                        <= DUPLICATE_WINDOW_MS
                        || neighbours_match(earlier, word))
            })
            .map(|(index, _)| index);
        match copy {
            None => written.push(word),
            Some(index) => {
                let weight_of = |speaker: &Speaker| weight.get(speaker).copied().unwrap_or(0);
                if weight_of(&word.speaker) > weight_of(&written[index].speaker) {
                    written[index] = word;
                }
            }
        }
    }
    written.sort_by_key(|word| (word.word.start_ms(), word.source));
    segments_of(
        meeting_id,
        written
            .into_iter()
            .map(|word| (&word.word, word.speaker, word.doubtful)),
    )
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
