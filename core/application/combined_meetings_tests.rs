//! Specifies how devices fold their recordings of one meeting together and what each does with
//! its retained speech afterwards (#88).

use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, TimeZone, Utc};

use super::combined_meetings::{
    AudioPlan, COMBINING_GRACE, KEEP_FOR_COMBINING, find_combinations, plan_retained_audio,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{DeviceId, MeetingId, TranscriptSegment},
    },
    ports::{meeting_document::MeetingCombination, storage::MeetingRecording},
};

/// Lines of one conversation, one per 4 s.
const TALK: [&str; 8] = [
    "Buongiorno a tutti, iniziamo con il bilancio del trimestre.",
    "Le vendite sono cresciute del dieci per cento rispetto a marzo.",
    "Però i costi di spedizione sono aumentati più del previsto.",
    "Dobbiamo rinegoziare il contratto con il corriere entro giugno.",
    "Chi se ne occupa? Io posso chiamarli già domani mattina.",
    "Perfetto, poi passiamo al lancio del nuovo prodotto.",
    "Il prototipo è pronto ma mancano ancora i test sul campo.",
    "Possiamo coinvolgere tre clienti di Milano per la prova.",
];

/// Lines of another conversation.
const OTHER_TALK: [&str; 8] = [
    "Allora, come è andata la trasferta a Torino la settimana scorsa?",
    "Bene, abbiamo visto due fornitori e uno sembra molto serio.",
    "Hanno mandato il preventivo per le scaffalature del magazzino?",
    "Non ancora, dicevano lunedì ma non è arrivato nulla.",
    "Allora sollecitali, ci servono prima della fiera di ottobre.",
    "D'accordo, e per il furgone nuovo hai sentito la banca?",
    "Sì, il finanziamento è approvato con un tasso fisso.",
    "Ottimo, firmiamo appena arriva la documentazione completa.",
];

fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + seconds, 0)
        .single()
        .unwrap_or_else(|| panic!("the instant must exist"))
}

fn recording(meeting: &str, device: &str, started_s: i64) -> MeetingRecording {
    MeetingRecording {
        meeting_id: MeetingId::new(meeting),
        created_at: at(started_s),
        acquisition_device_id: DeviceId::new(device),
        combined_into: None,
        kept_apart_from: Vec::new(),
    }
}

fn segments(meeting: &str, lines: &[&str], first_ms: u64) -> Vec<TranscriptSegment> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let start = first_ms + index as u64 * 4_000;
            TranscriptSegment::new(MeetingId::new(meeting), start, start + 3_800, None, *line)
        })
        .collect()
}

/// A loader over fixed transcripts.
struct Transcripts(HashMap<String, Vec<TranscriptSegment>>);

impl Transcripts {
    fn load(&self, meeting_id: &MeetingId) -> Result<Vec<TranscriptSegment>, StorageError> {
        Ok(self.0.get(meeting_id.as_str()).cloned().unwrap_or_default())
    }
}

/// The laptop started at 0 s and heard the talk 3 s in; the phone started 2 s later.
fn laptop_and_phone() -> (Vec<MeetingRecording>, Transcripts) {
    (
        vec![
            recording("meeting-laptop", "laptop", 0),
            recording("meeting-phone", "phone", 2),
        ],
        Transcripts(HashMap::from([
            (
                "meeting-laptop".to_owned(),
                segments("meeting-laptop", &TALK, 3_000),
            ),
            (
                "meeting-phone".to_owned(),
                segments("meeting-phone", &TALK, 1_000),
            ),
        ])),
    )
}

#[test]
fn the_later_recording_is_folded_into_the_earlier_and_a_desktop_combines() {
    let (recordings, transcripts) = laptop_and_phone();

    let found = find_combinations(
        &recordings,
        &|meeting| transcripts.load(meeting),
        &DeviceId::new("laptop"),
        false,
        at(-3_600),
    );

    let found = found.unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        found,
        vec![(
            MeetingId::new("meeting-phone"),
            MeetingCombination {
                into: MeetingId::new("meeting-laptop"),
                offset_ms: 2_000,
                combine_on: DeviceId::new("laptop"),
            }
        )]
    );
}

#[test]
fn a_phone_that_finds_the_pair_leaves_the_combining_to_the_other_device() {
    let (recordings, transcripts) = laptop_and_phone();

    let found = find_combinations(
        &recordings,
        &|meeting| transcripts.load(meeting),
        &DeviceId::new("phone"),
        true,
        at(-3_600),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].1.combine_on, DeviceId::new("laptop"));
}

#[test]
fn recordings_already_folded_split_or_of_other_devices_are_left_alone() {
    let (mut recordings, transcripts) = laptop_and_phone();
    let finds = |recordings: &[MeetingRecording], device: &str| {
        find_combinations(
            recordings,
            &|meeting| transcripts.load(meeting),
            &DeviceId::new(device),
            false,
            at(-3_600),
        )
        .unwrap_or_else(|error| panic!("{error}"))
    };

    // A third device holding both meetings does not decide for them.
    assert!(finds(&recordings, "home-server").is_empty());

    recordings[1].kept_apart_from = vec![MeetingId::new("meeting-laptop")];
    assert!(finds(&recordings, "laptop").is_empty());

    recordings[1].kept_apart_from.clear();
    recordings[1].combined_into = Some(MeetingCombination {
        into: MeetingId::new("meeting-laptop"),
        offset_ms: 2_000,
        combine_on: DeviceId::new("laptop"),
    });
    assert!(finds(&recordings, "laptop").is_empty());
}

#[test]
fn two_recordings_of_one_device_or_of_different_talks_are_not_joined() {
    let recordings = vec![
        recording("meeting-laptop", "laptop", 0),
        recording("meeting-phone", "phone", 2),
        recording("meeting-laptop-2", "laptop", 1),
    ];
    let transcripts = Transcripts(HashMap::from([
        (
            "meeting-laptop".to_owned(),
            segments("meeting-laptop", &TALK, 3_000),
        ),
        (
            "meeting-phone".to_owned(),
            segments("meeting-phone", &OTHER_TALK, 1_000),
        ),
        (
            "meeting-laptop-2".to_owned(),
            segments("meeting-laptop-2", &TALK, 2_000),
        ),
    ]));

    let found = find_combinations(
        &recordings,
        &|meeting| transcripts.load(meeting),
        &DeviceId::new("laptop"),
        false,
        at(-3_600),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(found.is_empty());
}

#[test]
fn recordings_older_than_the_window_are_not_compared() {
    let (recordings, transcripts) = laptop_and_phone();

    let found = find_combinations(
        &recordings,
        &|meeting| transcripts.load(meeting),
        &DeviceId::new("laptop"),
        false,
        at(60),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(found.is_empty());
}

/// The laptop's and the phone's recordings, the phone's folded into the laptop's.
fn folded(combine_on: &str) -> Vec<MeetingRecording> {
    let mut recordings = vec![
        recording("meeting-laptop", "laptop", 0),
        recording("meeting-phone", "phone", 2),
    ];
    recordings[1].combined_into = Some(MeetingCombination {
        into: MeetingId::new("meeting-laptop"),
        offset_ms: 2_000,
        combine_on: DeviceId::new(combine_on),
    });
    recordings
}

fn plan(
    meeting: &str,
    recordings: &[MeetingRecording],
    held: &[&str],
    device: &str,
    age: Duration,
    can_pass: bool,
) -> AudioPlan {
    plan_retained_audio(
        &MeetingId::new(meeting),
        recordings,
        &|meeting| held.contains(&meeting.as_str()),
        &DeviceId::new(device),
        age,
        can_pass,
    )
}

#[test]
fn a_meeting_alone_waits_for_a_partner_then_takes_the_pass_alone() {
    let recordings = vec![recording("meeting-laptop", "laptop", 0)];

    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &["meeting-laptop"],
            "laptop",
            Duration::from_secs(60),
            true
        ),
        AudioPlan::Wait
    );
    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &["meeting-laptop"],
            "laptop",
            COMBINING_GRACE,
            true
        ),
        AudioPlan::PassAlone
    );
}

#[test]
fn a_device_without_the_pass_keeps_its_speech_only_for_a_while() {
    let recordings = vec![recording("meeting-phone", "phone", 0)];

    assert_eq!(
        plan(
            "meeting-phone",
            &recordings,
            &["meeting-phone"],
            "phone",
            COMBINING_GRACE,
            false
        ),
        AudioPlan::Wait
    );
    assert_eq!(
        plan(
            "meeting-phone",
            &recordings,
            &["meeting-phone"],
            "phone",
            KEEP_FOR_COMBINING,
            false
        ),
        AudioPlan::Discard
    );
}

#[test]
fn the_device_that_does_not_combine_sends_its_speech() {
    let recordings = folded("laptop");

    assert_eq!(
        plan(
            "meeting-phone",
            &recordings,
            &["meeting-phone"],
            "phone",
            Duration::ZERO,
            false
        ),
        AudioPlan::Send {
            to: DeviceId::new("laptop")
        }
    );
}

#[test]
fn the_first_recording_is_sent_too_when_the_other_device_combines() {
    let recordings = folded("phone");

    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &["meeting-laptop"],
            "laptop",
            Duration::ZERO,
            true
        ),
        AudioPlan::Send {
            to: DeviceId::new("phone")
        }
    );
}

#[test]
fn the_combining_device_waits_for_every_recording_then_combines() {
    let recordings = folded("laptop");
    let combine = AudioPlan::Combine {
        root: MeetingId::new("meeting-laptop"),
        members: vec![(MeetingId::new("meeting-phone"), 2_000)],
    };

    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &["meeting-laptop"],
            "laptop",
            COMBINING_GRACE,
            true
        ),
        AudioPlan::Wait
    );
    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &["meeting-laptop", "meeting-phone"],
            "laptop",
            Duration::ZERO,
            true
        ),
        combine
    );
    // The received recording leads to the same combination.
    assert_eq!(
        plan(
            "meeting-phone",
            &recordings,
            &["meeting-laptop", "meeting-phone"],
            "laptop",
            Duration::ZERO,
            true
        ),
        combine
    );
}

#[test]
fn a_recording_that_never_arrives_leaves_the_pass_to_what_is_here() {
    let recordings = folded("laptop");

    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &["meeting-laptop"],
            "laptop",
            KEEP_FOR_COMBINING,
            true
        ),
        AudioPlan::Combine {
            root: MeetingId::new("meeting-laptop"),
            members: Vec::new(),
        }
    );
}

#[test]
fn speech_received_after_the_first_recording_was_done_is_discarded() {
    let recordings = folded("laptop");

    assert_eq!(
        plan(
            "meeting-phone",
            &recordings,
            &["meeting-phone"],
            "laptop",
            Duration::ZERO,
            true
        ),
        AudioPlan::Discard
    );
}
