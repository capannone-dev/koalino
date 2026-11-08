//! Specifies how two devices' recordings are recognised as one meeting (#88).

use super::same_meeting::{RecordingText, recognise_same_meeting};
use crate::domain::models::{MeetingId, TranscriptSegment};

/// A conversation long enough to recognise, as one sentence per 4-second segment.
const CONVERSATION: [&str; 10] = [
    "Buongiorno a tutti, iniziamo con il bilancio del trimestre.",
    "Le vendite sono cresciute del dieci per cento rispetto a marzo.",
    "Però i costi di spedizione sono aumentati più del previsto.",
    "Dobbiamo rinegoziare il contratto con il corriere entro giugno.",
    "Chi se ne occupa? Io posso chiamarli già domani mattina.",
    "Perfetto, poi passiamo al lancio del nuovo prodotto.",
    "Il prototipo è pronto ma mancano ancora i test sul campo.",
    "Possiamo coinvolgere tre clienti di Milano per la prova.",
    "Va bene, preparo una lista e la mando entro venerdì.",
    "Grazie a tutti, ci aggiorniamo la prossima settimana.",
];

/// Another conversation held at the same time in another room.
const OTHER_CONVERSATION: [&str; 10] = [
    "Allora, come è andata la trasferta a Torino la settimana scorsa?",
    "Bene, abbiamo visto due fornitori e uno sembra molto serio.",
    "Hanno mandato il preventivo per le scaffalature del magazzino?",
    "Non ancora, dicevano lunedì ma non è arrivato nulla.",
    "Allora sollecitali, ci servono prima della fiera di ottobre.",
    "D'accordo, e per il furgone nuovo hai sentito la banca?",
    "Sì, il finanziamento è approvato con un tasso fisso.",
    "Ottimo, firmiamo appena arriva la documentazione completa.",
    "Ricordati anche delle ferie del personale ad agosto.",
    "Certo, il calendario lo chiudiamo entro la fine del mese.",
];

/// Segments of `lines`, one every 4 s from `first_start_ms`.
fn segments(lines: &[&str], first_start_ms: u64) -> Vec<TranscriptSegment> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let start_ms = first_start_ms + index as u64 * 4_000;
            TranscriptSegment::new(
                MeetingId::new("meeting"),
                start_ms,
                start_ms + 3_800,
                None,
                *line,
            )
        })
        .collect()
}

/// The same lines as another device hears them: some words misheard, one line missed.
fn heard_by_other_device(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| match index {
            1 => line.replace("dieci", "dieci,").replace("marzo", "maggio"),
            4 => String::new(),
            7 => line.replace("Milano", "Milano?").replace("tre", "te"),
            _ => (*line).to_owned(),
        })
        .collect()
}

#[test]
fn two_devices_in_one_room_are_recognised_with_their_clock_offset() {
    // The talk starts 3 s into the laptop's recording. The phone was started 2.5 s after the
    // laptop, so it hears the talk 0.5 s in, and its clock is 300 ms fast.
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 3_000));
    let phone_lines = heard_by_other_device(&CONVERSATION);
    let phone_lines: Vec<&str> = phone_lines.iter().map(String::as_str).collect();
    let phone = RecordingText::from_segments(1_002_800, &segments(&phone_lines, 500));

    let found = recognise_same_meeting(&laptop, &phone);

    let found = found.unwrap_or_else(|| panic!("the same conversation was not recognised"));
    assert!(
        (found.offset_ms() - 2_500).abs() <= 300,
        "offset {} ms, expected about 2500 ms",
        found.offset_ms()
    );
}

#[test]
fn recognition_does_not_depend_on_which_device_is_first() {
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 2_000));
    let phone = RecordingText::from_segments(1_002_000, &segments(&CONVERSATION, 0));

    let forward = recognise_same_meeting(&laptop, &phone).map(|found| found.offset_ms());
    let backward = recognise_same_meeting(&phone, &laptop).map(|found| found.offset_ms());

    assert_eq!(forward, Some(2_000));
    assert_eq!(backward, Some(-2_000));
}

#[test]
fn two_meetings_held_at_the_same_time_are_kept_apart() {
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 0));
    let phone = RecordingText::from_segments(1_000_000, &segments(&OTHER_CONVERSATION, 0));

    assert!(recognise_same_meeting(&laptop, &phone).is_none());
}

#[test]
fn the_same_words_said_at_another_time_are_another_meeting() {
    // The same agenda read out an hour later on the phone.
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 0));
    let phone = RecordingText::from_segments(4_600_000, &segments(&CONVERSATION, 0));

    assert!(recognise_same_meeting(&laptop, &phone).is_none());
}

#[test]
fn a_few_shared_words_are_not_enough() {
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION[..1], 0));
    let phone = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION[..1], 0));

    assert!(recognise_same_meeting(&laptop, &phone).is_none());
}

#[test]
fn a_device_that_heard_only_part_of_the_meeting_still_joins_it() {
    // The phone was started late and heard only the last four lines, as they were said.
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 0));
    let phone = RecordingText::from_segments(1_024_000, &segments(&CONVERSATION[6..], 0));

    let found = recognise_same_meeting(&laptop, &phone);

    assert_eq!(found.map(|found| found.offset_ms()), Some(24_000));
}

#[test]
fn words_are_spread_over_their_segment_and_compared_without_case_or_punctuation() {
    let text = RecordingText::from_segments(
        0,
        &[TranscriptSegment::new(
            MeetingId::new("meeting"),
            1_000,
            2_000,
            None,
            "Sì, VA bene!",
        )],
    );

    let words: Vec<(u64, &str)> = text
        .words()
        .iter()
        .map(|word| (word.time_ms(), word.key()))
        .collect();
    assert_eq!(words, vec![(1_000, "sì"), (1_333, "va"), (1_666, "bene")]);
}

#[test]
fn a_recording_that_overlaps_the_other_only_briefly_is_another_meeting() {
    // The phone was started for the last 10 s of the meeting and kept recording the next
    // conversation for almost two minutes: what it shares is real, but the two recordings mostly
    // cover different times.
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 0));
    let mut phone_segments = segments(&CONVERSATION[8..], 2_000);
    phone_segments.extend(segments(&OTHER_CONVERSATION[..2], 100_000));
    let phone = RecordingText::from_segments(1_030_000, &phone_segments);

    assert!(recognise_same_meeting(&laptop, &phone).is_none());
}

#[test]
fn recordings_of_the_same_span_need_fewer_shared_words() {
    // The phone sat by the other person and the laptop by the owner, so they mostly understood
    // different words; over the same span of time, two shared lines are enough.
    // The phone was started 400 ms after the laptop.
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 400));
    let phone_lines: Vec<&str> = OTHER_CONVERSATION
        .iter()
        .enumerate()
        .map(|(index, line)| match index {
            2 | 5 => CONVERSATION[index],
            _ => *line,
        })
        .collect();
    let phone = RecordingText::from_segments(1_000_400, &segments(&phone_lines, 0));

    let found = recognise_same_meeting(&laptop, &phone);

    let found = found.unwrap_or_else(|| panic!("the same span was not recognised"));
    assert!(
        found.shared_share() < 0.25,
        "share {}",
        found.shared_share()
    );
    assert_eq!(found.offset_ms(), 400);
}

#[test]
fn recordings_of_the_same_span_still_need_some_shared_words() {
    let laptop = RecordingText::from_segments(1_000_000, &segments(&CONVERSATION, 0));
    let phone_lines: Vec<&str> = OTHER_CONVERSATION
        .iter()
        .enumerate()
        .map(|(index, line)| if index == 5 { CONVERSATION[5] } else { *line })
        .collect();
    let phone = RecordingText::from_segments(1_000_000, &segments(&phone_lines, 0));

    assert!(recognise_same_meeting(&laptop, &phone).is_none());
}
