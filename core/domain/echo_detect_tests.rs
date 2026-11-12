//! Specifies when echo counts as heard (#108): the other side's voice leaking into the microphone
//! at a steady lag is; two people talking over each other, a silent microphone, or too little
//! audio is not. Signals follow Yogurt's `app/test/echoDetect.test.ts` (FrigadeHQ/yogurt, MIT
//! License, Copyright (c) 2026 Frigade): speech-like bursts, and a leak at gain 0.3 after 60 ms.

use super::echo_detect::{EchoDetector, EchoTrack};

/// Samples per second.
const RATE: usize = 16_000;
/// Samples fed per push, as the recorder's 100 ms frames.
const STEP: usize = 1_600;

/// A small deterministic generator, so each seed is one speaker.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    /// Uniform in -1 to 1.
    fn noise(&mut self) -> f32 {
        (self.next() % 20_001) as f32 / 10_000.0 - 1.0
    }
}

/// Speech-like audio: noise in syllables of 120 to 400 ms with pauses of 80 to 500 ms between,
/// each syllable at its own loudness.
fn speech(seed: u64, seconds: usize) -> Vec<i16> {
    let mut random = Lcg(seed);
    let total = seconds * RATE;
    let mut samples = Vec::with_capacity(total);
    while samples.len() < total {
        let on = (120 + random.next() % 280) as usize * RATE / 1_000;
        let loudness = 2_000.0 + (random.next() % 6_000) as f32;
        for _ in 0..on {
            samples.push((random.noise() * loudness) as i16);
        }
        let off = (80 + random.next() % 420) as usize * RATE / 1_000;
        samples.extend(std::iter::repeat_n(0, off));
    }
    samples.truncate(total);
    samples
}

/// Their voice as the microphone hears it from the speakers: quieter and late.
fn leak(theirs: &[i16], gain: f32, delay_ms: usize) -> Vec<i16> {
    let delay = delay_ms * RATE / 1_000;
    (0..theirs.len())
        .map(|index| {
            index
                .checked_sub(delay)
                .map_or(0, |earlier| (f32::from(theirs[earlier]) * gain) as i16)
        })
        .collect()
}

fn mix(a: &[i16], b: &[i16]) -> Vec<i16> {
    a.iter().zip(b).map(|(x, y)| x.saturating_add(*y)).collect()
}

/// Feeds both tracks side by side in 100 ms steps, as a recording does.
fn feed(detector: &mut EchoDetector, microphone: &[i16], computer_sound: &[i16]) {
    for (index, (mine, theirs)) in microphone
        .chunks(STEP)
        .zip(computer_sound.chunks(STEP))
        .enumerate()
    {
        let start_ms = (index * STEP * 1_000 / RATE) as u64;
        detector.push(EchoTrack::ComputerSound, start_ms, theirs);
        detector.push(EchoTrack::Microphone, start_ms, mine);
    }
}

#[test]
fn their_voice_from_the_speakers_is_heard() {
    let theirs = speech(1, 60);
    let mut detector = EchoDetector::new();
    feed(&mut detector, &leak(&theirs, 0.3, 60), &theirs);
    assert!(detector.active_windows() >= 10);
    assert!(detector.present());
    assert!(detector.correlated_at(31_000));
}

#[test]
fn a_leak_under_the_owner_talking_is_heard_too() {
    let theirs = speech(1, 60);
    let own = speech(7, 60);
    let quiet_own: Vec<i16> = own.iter().map(|sample| sample / 4).collect();
    let mut detector = EchoDetector::new();
    feed(
        &mut detector,
        &mix(&leak(&theirs, 0.3, 60), &quiet_own),
        &theirs,
    );
    assert!(detector.present());
}

#[test]
fn two_people_talking_over_each_other_is_not_echo() {
    let theirs = speech(1, 60);
    let own = speech(2, 60);
    let mut detector = EchoDetector::new();
    feed(&mut detector, &own, &theirs);
    assert!(detector.active_windows() >= 10, "both sides carried sound");
    assert!(!detector.present());
}

#[test]
fn a_silent_microphone_or_too_little_audio_is_not_echo() {
    let theirs = speech(1, 60);
    let mut silent = EchoDetector::new();
    feed(&mut silent, &vec![0; theirs.len()], &theirs);
    assert_eq!(silent.active_windows(), 0);
    assert!(!silent.present());

    let short = speech(1, 12);
    let mut brief = EchoDetector::new();
    feed(&mut brief, &leak(&short, 0.3, 60), &short);
    assert!(!brief.present(), "under ten windows is no evidence");
}

#[test]
fn a_pause_in_the_recording_does_not_stop_the_checks() {
    let theirs = speech(3, 30);
    let mine = leak(&theirs, 0.3, 60);
    let mut detector = EchoDetector::new();
    feed(&mut detector, &mine[..10 * RATE], &theirs[..10 * RATE]);
    let before = detector.active_windows();
    // Ten minutes later the recording goes on.
    for (index, (mine, theirs)) in mine[10 * RATE..]
        .chunks(STEP)
        .zip(theirs[10 * RATE..].chunks(STEP))
        .enumerate()
    {
        let start_ms = 600_000 + (index * STEP * 1_000 / RATE) as u64;
        detector.push(EchoTrack::ComputerSound, start_ms, theirs);
        detector.push(EchoTrack::Microphone, start_ms, mine);
    }
    assert!(detector.active_windows() > before + 5);
    assert!(detector.present());
}

#[test]
fn the_microphone_arriving_later_than_their_track_is_still_compared() {
    let theirs = speech(4, 40);
    let mine = leak(&theirs, 0.3, 60);
    let mut detector = EchoDetector::new();
    for (index, chunk) in theirs.chunks(STEP).enumerate() {
        detector.push(EchoTrack::ComputerSound, (index * 100) as u64, chunk);
    }
    for (index, chunk) in mine.chunks(STEP).enumerate() {
        detector.push(EchoTrack::Microphone, (index * 100) as u64, chunk);
    }
    assert!(detector.present());
}
