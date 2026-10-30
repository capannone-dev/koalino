//! Decodes pyannote segmentation-3.0's powerset output (Issue #44).
//!
//! For each frame of a 10 s window the model scores seven classes covering at most three local
//! speakers, at most two of them at once, in pyannote's `Powerset(3, 2)` order: nobody; speaker
//! 1; 2; 3; 1 and 2; 1 and 3; 2 and 3. Local speaker numbers only mean something inside one
//! window. The highest score wins, so probabilities and log-probabilities decode alike.

/// Local speakers one window can hold.
pub const LOCAL_SPEAKERS: usize = 3;
/// Powerset classes per frame.
pub const POWERSET_CLASSES: usize = 7;

/// The local speakers each class activates, in pyannote's order.
const CLASS_SPEAKERS: [[bool; LOCAL_SPEAKERS]; POWERSET_CLASSES] = [
    [false, false, false],
    [true, false, false],
    [false, true, false],
    [false, false, true],
    [true, true, false],
    [true, false, true],
    [false, true, true],
];

/// Decodes one frame.
///
/// Inputs: The frame's seven class scores. Outputs: Which local speakers are active; nobody when
/// every score is NaN. Side effects: None.
pub fn active_speakers(scores: &[f32; POWERSET_CLASSES]) -> [bool; LOCAL_SPEAKERS] {
    let best_class = scores
        .iter()
        .enumerate()
        .filter(|(_, score)| !score.is_nan())
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map_or(0, |(class, _)| class);
    CLASS_SPEAKERS[best_class]
}
