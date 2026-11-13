//! Specifies how names on a call window become name suggestions for unknown voices (#113).

use uuid::Uuid;

use super::speaker_names::{
    NamedSpan, SpeakingTimeline, VoiceTurn, is_call_window, parse_voice_cues, speaking_name,
    suggest_names,
};

#[test]
fn speaking_labels_give_the_name() {
    assert_eq!(
        speaking_name("Alice Rossi is speaking").as_deref(),
        Some("Alice Rossi")
    );
    assert_eq!(speaking_name("Bruno is talking").as_deref(), Some("Bruno"));
    assert_eq!(
        speaking_name("Talking: Carla Neri").as_deref(),
        Some("Carla Neri")
    );
    assert_eq!(speaking_name("Speaking: Dario").as_deref(), Some("Dario"));
    assert_eq!(
        speaking_name("Elena Verdi, Speaking").as_deref(),
        Some("Elena Verdi")
    );
    assert_eq!(
        speaking_name("Franco Blu, video is off, speaking, muted").as_deref(),
        Some("Franco Blu")
    );
    assert_eq!(
        speaking_name("Gianni (speaking)").as_deref(),
        Some("Gianni")
    );
    assert_eq!(
        speaking_name("Ilaria sta parlando").as_deref(),
        Some("Ilaria")
    );
    assert_eq!(speaking_name("Sta parlando: Luca").as_deref(), Some("Luca"));
}

#[test]
fn other_labels_and_implausible_names_give_nothing() {
    assert_eq!(speaking_name("Mute"), None);
    assert_eq!(speaking_name("Alice Rossi, muted"), None);
    assert_eq!(speaking_name("Speaking"), None);
    assert_eq!(speaking_name("Speaking: "), None);
    assert_eq!(speaking_name("Someone is speaking"), None);
    assert_eq!(speaking_name("You are speaking"), None);
    assert_eq!(
        speaking_name("Raise your hand while someone is speaking"),
        None
    );
    let long_label = format!("{} is speaking", "word ".repeat(20));
    assert_eq!(speaking_name(&long_label), None);
}

#[test]
fn only_call_windows_are_read() {
    assert!(is_call_window("Weekly sync | Microsoft Teams"));
    assert!(is_call_window("Meet - abc-defg-hij — Mozilla Firefox"));
    assert!(is_call_window("Zoom Meeting"));
    assert!(is_call_window("Planning | Jitsi Meet"));
    assert!(is_call_window("Webex"));
    assert!(!is_call_window("Inbox - Mail — Mozilla Firefox"));
    assert!(!is_call_window("notes.txt - Mousepad"));
}

#[test]
fn readings_merge_into_spans_per_name() {
    let mut timeline = SpeakingTimeline::default();
    timeline.observe(10_000, &[String::from("Alice")]);
    timeline.observe(11_000, &[String::from("Alice")]);
    timeline.observe(12_000, &[String::from("Alice"), String::from("Bruno")]);
    timeline.observe(13_000, &[String::from("Bruno")]);
    // A gap longer than the merge limit starts a new span.
    timeline.observe(30_000, &[String::from("Alice")]);
    let spans = timeline.spans();
    assert_eq!(
        spans,
        vec![
            NamedSpan::new(9_000, 13_000, "Alice"),
            NamedSpan::new(11_000, 14_000, "Bruno"),
            NamedSpan::new(29_000, 31_000, "Alice"),
        ]
    );
}

#[test]
fn the_timeline_round_trips_through_webvtt() {
    let mut timeline = SpeakingTimeline::default();
    timeline.observe(3_723_000, &[String::from("Anna <B> & Co")]);
    let text = timeline.to_vtt();
    assert!(text.starts_with("WEBVTT\n"));
    assert!(text.contains("01:02:02.000 --> 01:02:04.000"));
    assert_eq!(
        parse_voice_cues(&text),
        vec![NamedSpan::new(3_722_000, 3_724_000, "Anna <B> & Co")]
    );
    assert!(SpeakingTimeline::default().is_empty());
}

#[test]
fn caption_cues_are_read_and_unnamed_cues_skipped() {
    let captions = "WEBVTT\n\n00:00:05.000 --> 00:00:07.500\n<v Alice Rossi>Hello there</v>\n\n\
                    00:00:08.000 --> 00:00:09.000\nno voice tag\n\n\
                    00:01:00.000 --> 00:01:02.000\n<v.loud Bruno>Ciao</v>\n\nbroken --> line\n";
    assert_eq!(
        parse_voice_cues(captions),
        vec![
            NamedSpan::new(5_000, 7_500, "Alice Rossi"),
            NamedSpan::new(60_000, 62_000, "Bruno"),
        ]
    );
    assert!(parse_voice_cues("not a vtt").is_empty());
}

fn turn(cluster: Uuid, start_ms: u64, end_ms: u64) -> VoiceTurn {
    VoiceTurn {
        cluster_id: cluster,
        start_ms,
        end_ms,
    }
}

#[test]
fn a_voice_is_suggested_the_name_shown_while_it_spoke() {
    let alice_voice = Uuid::new_v4();
    let bruno_voice = Uuid::new_v4();
    let turns = vec![
        turn(alice_voice, 0, 4_000),
        turn(bruno_voice, 5_000, 9_000),
        turn(alice_voice, 10_000, 14_000),
    ];
    let spans = vec![
        NamedSpan::new(0, 4_500, "Alice"),
        NamedSpan::new(4_800, 9_000, "Bruno"),
        NamedSpan::new(10_000, 14_000, "Alice"),
    ];
    let suggestions = suggest_names(&turns, &spans);
    assert_eq!(
        suggestions.get(&alice_voice).map(String::as_str),
        Some("Alice")
    );
    assert_eq!(
        suggestions.get(&bruno_voice).map(String::as_str),
        Some("Bruno")
    );
}

#[test]
fn no_suggestion_without_enough_or_clear_evidence() {
    let brief_voice = Uuid::new_v4();
    let mixed_voice = Uuid::new_v4();
    let silent_voice = Uuid::new_v4();
    let turns = vec![
        turn(brief_voice, 0, 1_000),
        turn(mixed_voice, 10_000, 20_000),
        turn(silent_voice, 40_000, 50_000),
    ];
    let spans = vec![
        NamedSpan::new(0, 1_000, "Alice"),
        NamedSpan::new(10_000, 15_000, "Bruno"),
        NamedSpan::new(15_000, 20_000, "Carla"),
    ];
    let suggestions = suggest_names(&turns, &spans);
    assert!(suggestions.is_empty(), "{suggestions:?}");
}

#[test]
fn names_differing_only_in_case_or_spaces_count_together() {
    let voice = Uuid::new_v4();
    let turns = vec![turn(voice, 0, 10_000)];
    let spans = vec![
        NamedSpan::new(0, 4_000, "Alice  Rossi"),
        NamedSpan::new(4_000, 8_000, "alice rossi"),
        NamedSpan::new(8_000, 10_000, "Bruno"),
    ];
    assert_eq!(
        suggest_names(&turns, &spans)
            .get(&voice)
            .map(String::as_str),
        Some("Alice Rossi")
    );
}

#[test]
fn a_name_that_wins_for_several_voices_is_suggested_to_none() {
    // A call window that is covered stops updating, so one stale label can span every voice.
    let first_voice = Uuid::new_v4();
    let second_voice = Uuid::new_v4();
    let named_voice = Uuid::new_v4();
    let turns = vec![
        turn(first_voice, 0, 5_000),
        turn(second_voice, 5_000, 10_000),
        turn(named_voice, 20_000, 25_000),
    ];
    let spans = vec![
        NamedSpan::new(0, 10_000, "Alice"),
        NamedSpan::new(20_000, 25_000, "Bruno"),
    ];
    let suggestions = suggest_names(&turns, &spans);
    assert_eq!(suggestions.get(&first_voice), None);
    assert_eq!(suggestions.get(&second_voice), None);
    assert_eq!(
        suggestions.get(&named_voice).map(String::as_str),
        Some("Bruno")
    );
}
