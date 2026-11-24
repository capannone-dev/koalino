//! Specifies how a muted or turned-off microphone is noticed: a few seconds of exact silence, or
//! no audio at all, while a real room's faint noise never counts.

use std::time::{Duration, Instant};

use super::microphone_silence::{MicrophoneSilence, SILENCE_NOTICE_AFTER, STALL_NOTICE_AFTER};

/// One second of 16 kHz mono audio, every sample `value`.
fn second_of(value: i16) -> Vec<i16> {
    vec![value; 16_000]
}

/// One second of faint room noise: small values that are not all zero.
fn second_of_room_noise() -> Vec<i16> {
    (0..16_000).map(|index| ((index % 7) as i16) - 3).collect()
}

#[test]
fn a_microphone_giving_only_zeros_is_noticed_after_a_few_seconds() {
    let opened = Instant::now();
    let mut silence = MicrophoneSilence::new(opened);
    let seconds = SILENCE_NOTICE_AFTER.as_secs();

    for second in 1..seconds {
        silence.observe(&second_of(0), opened + Duration::from_secs(second));
    }
    assert!(!silence.is_silent(opened + Duration::from_secs(seconds - 1)));

    silence.observe(&second_of(0), opened + Duration::from_secs(seconds));
    assert!(silence.is_silent(opened + Duration::from_secs(seconds)));
}

#[test]
fn faint_room_noise_is_not_silence() {
    let opened = Instant::now();
    let mut silence = MicrophoneSilence::new(opened);

    for second in 1..=10 {
        silence.observe(
            &second_of_room_noise(),
            opened + Duration::from_secs(second),
        );
    }

    assert!(!silence.is_silent(opened + Duration::from_secs(10)));
}

#[test]
fn sound_after_silence_clears_the_notice_at_once() {
    let opened = Instant::now();
    let mut silence = MicrophoneSilence::new(opened);
    for second in 1..=10 {
        silence.observe(&second_of(0), opened + Duration::from_secs(second));
    }
    assert!(silence.is_silent(opened + Duration::from_secs(10)));

    let mut unmuted = second_of(0);
    unmuted[15_999] = 400;
    silence.observe(&unmuted, opened + Duration::from_secs(11));

    assert!(!silence.is_silent(opened + Duration::from_secs(11)));
}

#[test]
fn a_microphone_that_delivers_nothing_is_noticed() {
    let opened = Instant::now();
    let silence = MicrophoneSilence::new(opened);

    assert!(!silence.is_silent(opened + STALL_NOTICE_AFTER - Duration::from_millis(1)));
    assert!(silence.is_silent(opened + STALL_NOTICE_AFTER));
}

#[test]
fn a_microphone_that_stops_delivering_is_noticed() {
    let opened = Instant::now();
    let mut silence = MicrophoneSilence::new(opened);
    silence.observe(&second_of_room_noise(), opened + Duration::from_secs(1));

    assert!(!silence.is_silent(opened + Duration::from_secs(1) + STALL_NOTICE_AFTER / 2));
    assert!(silence.is_silent(opened + Duration::from_secs(1) + STALL_NOTICE_AFTER));
}

#[test]
fn a_consent_store_value_of_deny_means_access_is_off() {
    let denied = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\\NonPackaged\r\n    Value    REG_SZ    Deny\r\n\r\n";
    let allowed = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone\r\n    Value    REG_SZ    Allow\r\n\r\n";

    assert!(super::microphone_silence::consent_value_denies(denied));
    assert!(!super::microphone_silence::consent_value_denies(allowed));
    assert!(!super::microphone_silence::consent_value_denies(""));
}
