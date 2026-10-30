//! Specifies the powerset decoding of pyannote segmentation-3.0 scores: seven classes for at most
//! three speakers per window, two of them at once, in pyannote's order.

use super::powerset::{LOCAL_SPEAKERS, POWERSET_CLASSES, active_speakers};

/// Every class maps to the speakers pyannote assigns it.
#[test]
fn classes_follow_pyannote_order() {
    let expected: [&[usize]; POWERSET_CLASSES] = [&[], &[0], &[1], &[2], &[0, 1], &[0, 2], &[1, 2]];

    for (class, speakers) in expected.iter().enumerate() {
        let mut scores = [0.0_f32; POWERSET_CLASSES];
        scores[class] = 1.0;
        let active = active_speakers(&scores);
        let listed: Vec<usize> = (0..LOCAL_SPEAKERS)
            .filter(|speaker| active[*speaker])
            .collect();
        assert_eq!(listed, speakers.to_vec(), "class {class}");
    }
}

/// The highest score wins, whether scores are probabilities or log-probabilities.
#[test]
fn the_highest_score_wins() {
    let log_probabilities = [-3.0, -0.1, -2.0, -4.0, -5.0, -6.0, -7.0];

    assert_eq!(active_speakers(&log_probabilities), [true, false, false]);
}
