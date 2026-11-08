//! Specifies how devices fold their recordings of one meeting together and what each does with
//! its retained speech afterwards (#88).

use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, TimeZone, Utc};

use super::combined_meetings::{
    AudioPlan, COMBINING_GRACE, HeldSpeech, KEEP_FOR_COMBINING, MeetingListing, PlanContext,
    combining_device, find_combinations, plan_retained_audio,
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
        recorded_on_phone: Some(device.contains("phone")),
        combined_with: Vec::new(),
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

/// The laptop's meeting with the phone's folded in, and a second phone that heard the other side
/// of the table, as the first phone did.
fn third_device(second_phone_started_s: i64) -> (Vec<MeetingRecording>, Transcripts) {
    let mut recordings = vec![
        recording("meeting-laptop", "laptop", 0),
        recording("meeting-phone", "phone", 2),
        recording("meeting-phone-2", "phone-2", second_phone_started_s),
    ];
    recordings[1].combined_into = Some(MeetingCombination {
        into: MeetingId::new("meeting-laptop"),
        offset_ms: 2_000,
        combine_on: DeviceId::new("laptop"),
    });
    let second_first_ms = u64::try_from(2_000 - second_phone_started_s * 1_000 + 1_000)
        .unwrap_or_else(|_| panic!("the second phone must start before it hears the talk"));
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
            "meeting-phone-2".to_owned(),
            segments("meeting-phone-2", &OTHER_TALK, second_first_ms),
        ),
    ]));
    (recordings, transcripts)
}

#[test]
fn a_third_device_joins_the_meeting_through_the_recording_it_resembles() {
    // The second phone shares nothing with the laptop, only with the first phone.
    let (recordings, transcripts) = third_device(1);

    let found = find_combinations(
        &recordings,
        &|meeting| transcripts.load(meeting),
        &DeviceId::new("phone-2"),
        true,
        at(-3_600),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        found,
        vec![(
            MeetingId::new("meeting-phone-2"),
            MeetingCombination {
                into: MeetingId::new("meeting-laptop"),
                offset_ms: 1_000,
                combine_on: DeviceId::new("laptop"),
            }
        )]
    );
}

#[test]
fn a_recording_started_before_the_meeting_joins_it_rather_than_becoming_its_first() {
    let (recordings, transcripts) = third_device(-1);

    let found = find_combinations(
        &recordings,
        &|meeting| transcripts.load(meeting),
        &DeviceId::new("phone-2"),
        true,
        at(-3_600),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, MeetingId::new("meeting-phone-2"));
    assert_eq!(found[0].1.into, MeetingId::new("meeting-laptop"));
    assert_eq!(found[0].1.offset_ms, -1_000);
}

#[test]
fn a_recording_split_from_one_recording_of_a_meeting_does_not_rejoin_it_through_another() {
    let (mut recordings, transcripts) = third_device(1);
    recordings[2].kept_apart_from = vec![MeetingId::new("meeting-laptop")];

    let found = find_combinations(
        &recordings,
        &|meeting| transcripts.load(meeting),
        &DeviceId::new("phone-2"),
        true,
        at(-3_600),
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

/// Speech held on the device being planned for: the meeting, how long ago it was written, and
/// whether its pass alone already ran.
type Held<'held> = &'held [(&'held str, Duration, bool)];

fn plan_with(
    meeting: &str,
    recordings: &[MeetingRecording],
    held: Held<'_>,
    device: &str,
    can_pass: bool,
    keeps_for_partners: bool,
) -> AudioPlan {
    let this_device = DeviceId::new(device);
    let held_speech = |meeting: &MeetingId| {
        held.iter()
            .find(|(held_meeting, _, _)| *held_meeting == meeting.as_str())
            .map(|(_, age, passed_alone)| HeldSpeech {
                age: *age,
                passed_alone: *passed_alone,
            })
    };
    plan_retained_audio(
        &MeetingId::new(meeting),
        recordings,
        &PlanContext {
            this_device: &this_device,
            can_pass,
            keeps_for_partners,
            held: &held_speech,
        },
    )
}

/// Plans on a device with P2P, so speech may wait for partners.
fn plan(
    meeting: &str,
    recordings: &[MeetingRecording],
    held: Held<'_>,
    device: &str,
    can_pass: bool,
) -> AudioPlan {
    plan_with(meeting, recordings, held, device, can_pass, true)
}

const MINUTE: Duration = Duration::from_secs(60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

fn combine(members: &[(&str, i64)]) -> AudioPlan {
    AudioPlan::Combine {
        root: MeetingId::new("meeting-laptop"),
        members: members
            .iter()
            .map(|(member, offset)| (MeetingId::new(*member), *offset))
            .collect(),
    }
}

#[test]
fn a_meeting_alone_takes_the_pass_then_keeps_its_speech_for_a_late_partner() {
    let recordings = vec![recording("meeting-laptop", "laptop", 0)];
    let laptop = |age, passed| {
        plan(
            "meeting-laptop",
            &recordings,
            &[("meeting-laptop", age, passed)],
            "laptop",
            true,
        )
    };

    assert_eq!(laptop(MINUTE, false), AudioPlan::Wait);
    assert_eq!(laptop(COMBINING_GRACE, false), AudioPlan::PassAlone);
    assert_eq!(laptop(20 * DAY, true), AudioPlan::Wait);
    assert_eq!(laptop(KEEP_FOR_COMBINING, true), AudioPlan::Discard);
}

#[test]
fn without_p2p_a_meeting_alone_keeps_nothing_after_its_pass() {
    let recordings = vec![recording("meeting-laptop", "laptop", 0)];

    assert_eq!(
        plan_with(
            "meeting-laptop",
            &recordings,
            &[("meeting-laptop", COMBINING_GRACE, true)],
            "laptop",
            true,
            false
        ),
        AudioPlan::Discard
    );
}

#[test]
fn a_device_without_the_pass_keeps_its_speech_only_for_a_while() {
    let recordings = vec![recording("meeting-phone", "phone", 0)];
    let phone = |age| {
        plan(
            "meeting-phone",
            &recordings,
            &[("meeting-phone", age, false)],
            "phone",
            false,
        )
    };

    assert_eq!(phone(COMBINING_GRACE), AudioPlan::Wait);
    assert_eq!(phone(KEEP_FOR_COMBINING), AudioPlan::Discard);
}

#[test]
fn the_device_that_does_not_combine_sends_its_speech_for_three_weeks() {
    let recordings = folded("laptop");
    let phone = |age| {
        plan(
            "meeting-phone",
            &recordings,
            &[("meeting-phone", age, true)],
            "phone",
            true,
        )
    };

    assert_eq!(
        phone(20 * DAY),
        AudioPlan::Send {
            to: DeviceId::new("laptop")
        }
    );
    assert_eq!(phone(KEEP_FOR_COMBINING), AudioPlan::Discard);
}

#[test]
fn the_combining_device_takes_the_pass_alone_while_it_waits_then_combines_late_speech() {
    let recordings = folded("laptop");
    let laptop = |held: Held<'_>, meeting: &str| plan(meeting, &recordings, held, "laptop", true);

    assert_eq!(
        laptop(&[("meeting-laptop", MINUTE, false)], "meeting-laptop"),
        AudioPlan::Wait
    );
    assert_eq!(
        laptop(
            &[("meeting-laptop", COMBINING_GRACE, false)],
            "meeting-laptop"
        ),
        AudioPlan::PassAlone
    );
    assert_eq!(
        laptop(&[("meeting-laptop", 5 * DAY, true)], "meeting-laptop"),
        AudioPlan::Wait
    );
    // The phone's speech arrives a week later.
    let both: Held<'_> = &[
        ("meeting-laptop", 7 * DAY, true),
        ("meeting-phone", Duration::ZERO, false),
    ];
    assert_eq!(
        laptop(both, "meeting-laptop"),
        combine(&[("meeting-phone", 2_000)])
    );
    assert_eq!(
        laptop(both, "meeting-phone"),
        combine(&[("meeting-phone", 2_000)])
    );
}

#[test]
fn a_received_recording_waits_while_the_first_takes_its_pass_alone() {
    let recordings = vec![
        recording("meeting-laptop", "laptop", 0),
        recording("meeting-phone", "phone", 2),
        recording("meeting-tablet", "tablet-phone", 3),
    ];
    let mut recordings = recordings;
    for member in [1, 2] {
        recordings[member].combined_into = folded("laptop")[1].combined_into.clone();
    }

    assert_eq!(
        plan(
            "meeting-phone",
            &recordings,
            &[
                ("meeting-laptop", COMBINING_GRACE, false),
                ("meeting-phone", MINUTE, false)
            ],
            "laptop",
            true
        ),
        AudioPlan::Wait
    );
}

#[test]
fn a_recording_that_never_arrives_leaves_the_pass_to_what_is_here() {
    let recordings = folded("laptop");

    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &[("meeting-laptop", KEEP_FOR_COMBINING, false)],
            "laptop",
            true
        ),
        combine(&[])
    );
    assert_eq!(
        plan(
            "meeting-laptop",
            &recordings,
            &[("meeting-laptop", KEEP_FOR_COMBINING, true)],
            "laptop",
            true
        ),
        AudioPlan::Discard
    );
}

#[test]
fn speech_received_after_the_first_recording_was_done_is_discarded() {
    let recordings = folded("laptop");

    assert_eq!(
        plan(
            "meeting-phone",
            &recordings,
            &[("meeting-phone", Duration::ZERO, false)],
            "laptop",
            true
        ),
        AudioPlan::Discard
    );
}

/// A phone's recording first, then the laptop's and a second phone's, both folded into it.
fn phone_first() -> Vec<MeetingRecording> {
    let mut recordings = vec![
        recording("meeting-phone", "phone", 0),
        recording("meeting-laptop", "laptop", 2),
        recording("meeting-phone-2", "phone-2", 3),
    ];
    for (member, combine_on) in [(1, "laptop"), (2, "phone")] {
        recordings[member].combined_into = Some(MeetingCombination {
            into: MeetingId::new("meeting-phone"),
            offset_ms: 2_000,
            combine_on: DeviceId::new(combine_on),
        });
    }
    recordings
}

#[test]
fn every_device_agrees_the_first_device_that_is_not_a_phone_combines() {
    let recordings = phone_first();

    assert_eq!(
        combining_device(&MeetingId::new("meeting-phone"), &recordings),
        Some(DeviceId::new("laptop"))
    );
    for (meeting, device) in [("meeting-phone", "phone"), ("meeting-phone-2", "phone-2")] {
        assert_eq!(
            plan(
                meeting,
                &recordings,
                &[(meeting, MINUTE, false)],
                device,
                true
            ),
            AudioPlan::Send {
                to: DeviceId::new("laptop")
            },
            "{device}"
        );
    }
}

#[test]
fn phones_alone_combine_on_the_first_one_and_older_meetings_keep_their_choice() {
    let mut recordings = phone_first();
    recordings[1].recorded_on_phone = Some(true);
    assert_eq!(
        combining_device(&MeetingId::new("meeting-phone"), &recordings),
        Some(DeviceId::new("phone"))
    );

    recordings[1].recorded_on_phone = None;
    assert_eq!(
        combining_device(&MeetingId::new("meeting-phone"), &recordings),
        Some(DeviceId::new("laptop")),
        "without every device's kind, the first fold's choice stands"
    );
}

#[test]
fn a_folded_recording_is_listed_once_under_the_first_with_both_devices() {
    let mut recordings = folded("laptop");
    recordings.push(recording("meeting-alone", "laptop", 9_000));

    let listing = MeetingListing::of(&recordings);

    assert!(listing.is_listed(&MeetingId::new("meeting-laptop")));
    assert!(!listing.is_listed(&MeetingId::new("meeting-phone")));
    assert!(listing.is_listed(&MeetingId::new("meeting-alone")));
    assert_eq!(
        listing.recorded_on(&MeetingId::new("meeting-laptop")),
        vec![DeviceId::new("laptop"), DeviceId::new("phone")]
    );
    assert_eq!(
        listing.folded_into(&MeetingId::new("meeting-laptop")),
        vec![MeetingId::new("meeting-phone")]
    );
    assert_eq!(
        listing.recorded_on(&MeetingId::new("meeting-alone")),
        vec![DeviceId::new("laptop")]
    );
    assert!(
        listing
            .folded_into(&MeetingId::new("meeting-alone"))
            .is_empty()
    );
}

#[test]
fn a_recording_folded_into_a_missing_meeting_is_listed_alone() {
    let recordings = vec![folded("laptop").remove(1)];

    let listing = MeetingListing::of(&recordings);

    assert!(listing.is_listed(&MeetingId::new("meeting-phone")));
    assert_eq!(
        listing.recorded_on(&MeetingId::new("meeting-phone")),
        vec![DeviceId::new("phone")]
    );
}
