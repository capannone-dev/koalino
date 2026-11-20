//! Specifies how one meeting recorded on two devices is transcribed from both (#88).

use uuid::Uuid;

use super::{
    combined_refinement::{SourceWords, combine_sources, computer_sound_on_first, refine_offset},
    transcript_refinement::MeetingWord,
};
use crate::domain::models::{MeetingId, SegmentSource, TranscriptSegment};

fn meeting() -> MeetingId {
    MeetingId::new("meeting-laptop")
}

fn segment(start_ms: u64, end_ms: u64, speaker: Uuid) -> TranscriptSegment {
    TranscriptSegment::new(meeting(), start_ms, end_ms, Some(speaker), "live")
}

/// Words of `text`, 400 ms each from `start_ms`.
fn words(text: &str, start_ms: u64) -> Vec<MeetingWord> {
    text.split_whitespace()
        .enumerate()
        .map(|(index, word)| {
            let start = start_ms + index as u64 * 400;
            MeetingWord::new(word, start, start + 350)
        })
        .collect()
}

fn texts(segments: &[TranscriptSegment]) -> Vec<(Option<Uuid>, String)> {
    segments
        .iter()
        .map(|segment| (segment.speaker_identity_id(), segment.text().to_owned()))
        .collect()
}

/// The owner (near the laptop) talks from 0 to 4 s, the guest (near the phone) from 5 to 9 s.
/// Each device hears its own person fully and the other one badly. The phone started 2 s after
/// the laptop, so its times are 2 s lower.
#[test]
fn each_person_is_taken_from_the_device_near_them() {
    let owner = Uuid::from_u128(1);
    let guest = Uuid::from_u128(2);
    let laptop_speakers = [segment(0, 4_000, owner), segment(5_000, 9_000, guest)];
    let phone_speakers = [
        segment(0, 2_000, Uuid::from_u128(11)),
        segment(3_000, 7_000, Uuid::from_u128(12)),
    ];
    let mut laptop_words = words("allora partiamo dal bilancio di marzo", 0);
    laptop_words.extend(words("sì", 5_000));
    let mut phone_words = words("partiamo bilancio", 0);
    phone_words.extend(words("sì però le spedizioni costano troppo", 3_000));

    let combined = combine_sources(
        &meeting(),
        &SourceWords {
            words: laptop_words,
            offset_ms: 0,
            speakers: &laptop_speakers,
        },
        &[SourceWords {
            words: phone_words,
            offset_ms: 2_000,
            speakers: &phone_speakers,
        }],
    );

    assert_eq!(
        texts(&combined),
        vec![
            (
                Some(owner),
                "allora partiamo dal bilancio di marzo".to_owned()
            ),
            (
                Some(guest),
                "sì però le spedizioni costano troppo".to_owned()
            ),
        ]
    );
    assert_eq!(
        combined[1].start_time_ms(),
        5_000,
        "phone time moved onto the laptop's"
    );
}

/// When both talk at once, each device keeps the words of the person near it.
#[test]
fn overlapping_speech_keeps_both_people() {
    let owner = Uuid::from_u128(1);
    let guest = Uuid::from_u128(2);
    // The guest objects (3.0-4.6 s) while the owner goes on. The laptop's speaker pass hears the
    // owner through most of it; the phone's hears the guest.
    let laptop_speakers = [
        segment(0, 3_000, owner),
        segment(3_000, 3_500, guest),
        segment(3_500, 5_000, owner),
    ];
    let phone_speakers = [segment(0, 3_000, owner), segment(3_000, 6_000, guest)];
    let mut laptop_words = words("io direi di aspettare", 0);
    laptop_words.extend(words("no", 3_000));
    laptop_words.extend(words("fino a lunedì", 3_600));
    let mut phone_words = words("direi", 400);
    phone_words.extend(words("no non sono d'accordo", 3_000));

    let combined = combine_sources(
        &meeting(),
        &SourceWords {
            words: laptop_words,
            offset_ms: 0,
            speakers: &laptop_speakers,
        },
        &[SourceWords {
            words: phone_words,
            offset_ms: 0,
            speakers: &phone_speakers,
        }],
    );

    let said_by = |person: Uuid| -> String {
        combined
            .iter()
            .filter(|segment| segment.speaker_identity_id() == Some(person))
            .map(TranscriptSegment::text)
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(said_by(owner), "io direi di aspettare fino a lunedì");
    assert_eq!(said_by(guest), "no non sono d'accordo");
}

/// A word each device gives a different person, each taken from that device, is written once.
#[test]
fn a_word_heard_by_both_devices_is_written_once() {
    let owner = Uuid::from_u128(1);
    let guest = Uuid::from_u128(2);
    let laptop_speakers = [segment(0, 3_000, owner), segment(3_000, 9_000, guest)];
    // The phone's speaker pass gave the owner's last word to the guest.
    let phone_speakers = [segment(0, 1_000, owner), segment(1_000, 9_000, guest)];
    let mut laptop_words = words("buongiorno a tutti grazie", 0);
    laptop_words.extend(words("prego", 4_000));
    let mut phone_words = words("buongiorno", 0);
    phone_words.extend(words("grazie", 1_250));
    phone_words.extend(words("prego a lei", 4_000));

    let combined = combine_sources(
        &meeting(),
        &SourceWords {
            words: laptop_words,
            offset_ms: 0,
            speakers: &laptop_speakers,
        },
        &[SourceWords {
            words: phone_words,
            offset_ms: 0,
            speakers: &phone_speakers,
        }],
    );

    assert_eq!(
        texts(&combined),
        vec![
            (Some(owner), "buongiorno a tutti grazie".to_owned()),
            (Some(guest), "prego a lei".to_owned()),
        ]
    );
}

/// The fold's offset comes from transcripts timed by segment; words timed by the pass refine it.
#[test]
fn the_offset_is_refined_from_the_words_both_devices_heard() {
    let talk = "allora partiamo dal bilancio di marzo poi passiamo alle spedizioni \
                che quest'anno costano molto di più e dobbiamo rinegoziare il contratto \
                con il corriere entro la fine di giugno";
    let laptop = words(talk, 10_000);
    let phone = words(talk, 7_640);

    assert_eq!(refine_offset(&laptop, &phone, 3_000), 2_360);
    assert_eq!(
        refine_offset(&laptop, &words("tutt'altra conversazione qui", 0), 3_000),
        3_000,
        "nothing shared keeps the fold's offset"
    );
}

/// The computer's sound each recording heard is kept, on the first recording's timeline, and the
/// microphone's live text is left to the pass (#108).
#[test]
fn the_computer_sound_of_every_recording_is_kept_on_the_first_timeline() {
    let heard = |meeting: &str, start_ms: u64, text: &str| {
        TranscriptSegment::new(
            MeetingId::new(meeting),
            start_ms,
            start_ms + 1_000,
            None,
            text,
        )
        .with_source(SegmentSource::ComputerSound)
    };
    let laptop = vec![
        segment(0, 1_000, Uuid::from_u128(1)),
        heard("meeting-laptop", 2_000, "their words"),
    ];
    let phone = vec![heard("meeting-phone", 500, "more of theirs")];

    let kept = computer_sound_on_first(&meeting(), &[(&laptop, 0), (&phone, 2_000)]);

    let placed: Vec<(u64, &str, &str)> = kept
        .iter()
        .map(|segment| {
            (
                segment.start_time_ms(),
                segment.text(),
                segment.meeting_id().as_str(),
            )
        })
        .collect();
    assert_eq!(
        placed,
        vec![
            (2_000, "their words", "meeting-laptop"),
            (2_500, "more of theirs", "meeting-laptop"),
        ]
    );
    assert!(
        kept.iter()
            .all(|segment| segment.source() == SegmentSource::ComputerSound)
    );
}

/// A phrase both devices wrote more than a second apart is written once, by the device whose
/// speaker carries the sentence: one device's speaker pass gave two words of the guest's sentence
/// to a speaker of their own, so each device's copy was chosen for a different speaker (#88).
#[test]
fn a_phrase_both_devices_wrote_apart_is_written_once_in_the_main_voice() {
    let guest = Uuid::from_u128(2);
    let split = Uuid::from_u128(3);
    let laptop_speakers = [
        segment(3_000, 5_000, guest),
        segment(5_000, 6_000, split),
        segment(7_000, 9_000, guest),
    ];
    let phone_speakers = [segment(3_000, 9_000, Uuid::from_u128(12))];
    let mut laptop_words = words("l'incidente è avvenuto in", 3_000);
    laptop_words.extend(words("alta montagna", 5_000));
    laptop_words.extend(words("e si ritiene", 7_400));
    let mut phone_words = words("l'incidente è avvenuto in", 3_000);
    phone_words.extend(words("alta montagna e si ritiene", 6_200));

    let combined = combine_sources(
        &meeting(),
        &SourceWords {
            words: laptop_words,
            offset_ms: 0,
            speakers: &laptop_speakers,
        },
        &[SourceWords {
            words: phone_words,
            offset_ms: 0,
            speakers: &phone_speakers,
        }],
    );

    assert_eq!(
        texts(&combined),
        vec![(
            Some(guest),
            "l'incidente è avvenuto in alta montagna e si ritiene".to_owned()
        )]
    );
}

/// Two people saying the same short word a moment apart are both written: one word alone, with
/// no neighbour in common, is not a copy.
#[test]
fn the_same_word_from_two_people_a_moment_apart_is_kept_twice() {
    let owner = Uuid::from_u128(1);
    let guest = Uuid::from_u128(2);
    let laptop_speakers = [segment(0, 1_500, owner), segment(1_500, 4_000, guest)];
    let phone_speakers = [segment(0, 1_500, owner), segment(1_500, 4_000, guest)];
    let laptop_words = words("va bene sì", 0);
    let mut phone_words = words("bene", 400);
    phone_words.extend(words("sì certo", 2_000));

    let combined = combine_sources(
        &meeting(),
        &SourceWords {
            words: laptop_words,
            offset_ms: 0,
            speakers: &laptop_speakers,
        },
        &[SourceWords {
            words: phone_words,
            offset_ms: 0,
            speakers: &phone_speakers,
        }],
    );

    assert_eq!(
        texts(&combined),
        vec![
            (Some(owner), "va bene sì".to_owned()),
            (Some(guest), "sì certo".to_owned()),
        ]
    );
}
