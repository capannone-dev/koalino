//! Specifies how the microphone falls back through its ways in (#152): the first that opens wins,
//! later ones are not tried, and when none opens the reason names every attempt and how to fix it.

use std::cell::Cell;

use super::microphone_fallback::{MICROPHONE_FIX_HINT, MicrophoneAttempt, open_first};

#[test]
fn the_first_way_in_that_opens_is_used_and_later_ones_are_not_tried() {
    let later_tried = Cell::new(false);
    let attempts: Vec<MicrophoneAttempt<'_, u8>> = vec![
        ("ALSA default", Box::new(|| Ok(1))),
        (
            "PulseAudio",
            Box::new(|| {
                later_tried.set(true);
                Ok(2)
            }),
        ),
    ];

    assert_eq!(
        open_first(attempts, MICROPHONE_FIX_HINT),
        Ok((1, "ALSA default"))
    );
    assert!(!later_tried.get());
}

#[test]
fn a_failed_way_in_falls_back_to_the_next() {
    let attempts: Vec<MicrophoneAttempt<'_, u8>> = vec![
        (
            "ALSA default",
            Box::new(|| Err(String::from("audio open error: No such file or directory"))),
        ),
        ("PulseAudio", Box::new(|| Ok(2))),
    ];

    assert_eq!(
        open_first(attempts, MICROPHONE_FIX_HINT),
        Ok((2, "PulseAudio"))
    );
}

#[test]
fn when_nothing_opens_the_reason_names_each_attempt_and_the_fix() {
    let attempts: Vec<MicrophoneAttempt<'_, u8>> = vec![
        ("ALSA default", Box::new(|| Err(String::from("no input")))),
        ("PulseAudio", Box::new(|| Err(String::from("no server")))),
    ];

    let reason = open_first(attempts, MICROPHONE_FIX_HINT)
        .err()
        .unwrap_or_default();

    assert!(reason.contains("ALSA default: no input"), "{reason}");
    assert!(reason.contains("PulseAudio: no server"), "{reason}");
    assert!(reason.ends_with(MICROPHONE_FIX_HINT), "{reason}");
}

#[test]
fn with_no_way_in_at_all_the_reason_still_says_so() {
    let reason = open_first(Vec::<MicrophoneAttempt<'_, u8>>::new(), MICROPHONE_FIX_HINT)
        .err()
        .unwrap_or_default();

    assert!(
        reason.starts_with("no microphone could be opened"),
        "{reason}"
    );
}
