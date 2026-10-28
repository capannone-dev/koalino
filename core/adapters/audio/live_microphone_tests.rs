//! Specifies `LiveMicrophone`: frames arrive in order, closing returns the tail and turns the
//! microphone off, and an open failure is reported without leaving anything running.

use std::time::Duration;

use crate::{
    adapters::audio::{fake_microphone::FakeMicrophone, live_microphone::LiveMicrophone},
    domain::errors::AudioError,
};

/// Opens a working fake microphone.
fn open(fake: &FakeMicrophone) -> LiveMicrophone {
    LiveMicrophone::open(fake).unwrap_or_else(|error| panic!("the fake must open: {error}"))
}

#[test]
fn frames_are_drained_in_order() {
    let fake = FakeMicrophone::working();
    let microphone = open(&fake);

    fake.speak(vec![1, 2]);
    fake.speak(vec![3]);

    assert_eq!(microphone.drain(), vec![vec![1, 2], vec![3]]);
    assert!(microphone.drain().is_empty());
}

#[test]
fn closing_turns_the_microphone_off_and_returns_the_tail() {
    let fake = FakeMicrophone::working();
    let microphone = open(&fake);
    fake.speak(vec![7, 8, 9]);

    let tail = microphone.close();

    assert_eq!(tail, vec![vec![7, 8, 9]]);
    assert_eq!(fake.counts(), (1, 1));
}

#[test]
fn waiting_for_frames_times_out_with_nothing() {
    let fake = FakeMicrophone::working();
    let microphone = open(&fake);

    assert!(
        microphone
            .wait_for_frames(Duration::from_millis(10))
            .is_empty()
    );
    fake.speak(vec![4]);
    assert_eq!(
        microphone.wait_for_frames(Duration::from_millis(10)),
        vec![vec![4]]
    );
}

#[test]
fn an_open_failure_is_reported() {
    let fake = FakeMicrophone::failing();

    let result = LiveMicrophone::open(&fake);

    assert!(matches!(result, Err(AudioError::Unavailable { .. })));
    assert_eq!(fake.log().open_count, 0);
}
