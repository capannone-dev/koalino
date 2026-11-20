//! Replays the combining of one meeting recorded on two devices from recorded inputs (#88), to
//! tune `combine_sources` on real data. Ignored: it needs a replay file.
//!
//! Input (`NOTE_TAKER_REPLAY_INPUT`): JSON `{"root": Source, "member": Source}` where a Source is
//! `{"words": [[text, start_ms, end_ms], ...], "speakers": [[start_ms, end_ms, label], ...],
//! "fold_offset_ms": n}`. Output (`NOTE_TAKER_REPLAY_OUTPUT`): JSON `[[start_ms, end_ms, label,
//! text], ...]`, the label being the root's speaker label (or 100 + the member's when unmapped).

use std::{env, fs};

use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    combined_refinement::{SourceWords, combine_sources, refine_offset},
    transcript_refinement::MeetingWord,
};
use crate::domain::models::{MeetingId, TranscriptSegment};

/// Reads one source's words and speakers; `label_base` keeps the two devices' labels apart.
fn source(value: &Value, label_base: u128) -> (Vec<MeetingWord>, Vec<TranscriptSegment>, i64) {
    let number = |item: &Value| item.as_u64().unwrap_or_default();
    let words = value["words"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|word| {
                    MeetingWord::new(
                        word[0].as_str().unwrap_or_default(),
                        number(&word[1]),
                        number(&word[2]),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let speakers = value["speakers"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|turn| {
                    TranscriptSegment::new(
                        MeetingId::new("replay"),
                        number(&turn[0]),
                        number(&turn[1]),
                        Some(Uuid::from_u128(
                            label_base + u128::from(number(&turn[2])) + 1,
                        )),
                        "replay",
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    (
        words,
        speakers,
        value["fold_offset_ms"].as_i64().unwrap_or_default(),
    )
}

/// Replays the combining step on the input file and writes the segments it gives.
#[test]
#[ignore = "needs a replay file"]
fn replay_combined_meeting() {
    let read = |name: &str| env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let input: Value = serde_json::from_slice(
        &fs::read(read("NOTE_TAKER_REPLAY_INPUT")).unwrap_or_else(|error| panic!("{error}")),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let (root_words, root_speakers, _) = source(&input["root"], 0);
    let (member_words, member_speakers, fold_offset_ms) = source(&input["member"], 1_000);
    let offset_ms = refine_offset(&root_words, &member_words, fold_offset_ms);
    let combined = combine_sources(
        &MeetingId::new("replay"),
        &SourceWords {
            words: root_words,
            offset_ms: 0,
            speakers: &root_speakers,
        },
        &[SourceWords {
            words: member_words,
            offset_ms,
            speakers: &member_speakers,
        }],
    );
    let rows: Vec<Value> = combined
        .iter()
        .map(|segment| {
            let label = segment
                .speaker_identity_id()
                .map_or(-1, |id| i64::try_from(id.as_u128()).unwrap_or(-1) - 1);
            json!([
                segment.start_time_ms(),
                segment.end_time_ms(),
                label,
                segment.text()
            ])
        })
        .collect();
    fs::write(
        read("NOTE_TAKER_REPLAY_OUTPUT"),
        json!({"offset_ms": offset_ms, "segments": rows}).to_string(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
}
