//! Specifies the slide-change heuristic (Issue #49): a frame becomes a slide once it is stable
//! and differs from the last saved slide; a cursor or compression noise is not a new slide, while
//! a new line of text is. Synthetic 1280x720 luma frames only.

use crate::{
    application::slide_change_detector::SlideChangeDetector, ports::screen_share::ScreenFrame,
};

const WIDTH: usize = 1_280;
const HEIGHT: usize = 720;

/// A white slide with dark text bars covering the rows in `text_rows` (pixel ranges).
fn slide(text_rows: &[(usize, usize)]) -> ScreenFrame {
    let mut luma = vec![240_u8; WIDTH * HEIGHT];
    for &(top, bottom) in text_rows {
        for row in top..bottom {
            luma[row * WIDTH + 100..row * WIDTH + 1_000].fill(20);
        }
    }
    frame(luma)
}

/// Wraps luma values in a frame with a placeholder image.
fn frame(luma: Vec<u8>) -> ScreenFrame {
    ScreenFrame::new(WIDTH, HEIGHT, luma, b"\x89PNG".to_vec())
        .unwrap_or_else(|error| panic!("test frame should be valid: {error}"))
}

/// Feeds `frames` in order and returns which ones the detector chose to save.
fn saved(detector: &mut SlideChangeDetector, frames: &[ScreenFrame]) -> Vec<bool> {
    frames.iter().map(|frame| detector.observe(frame)).collect()
}

/// Verifies a slide is saved on its second sample (once stable), and only once.
#[test]
fn a_slide_is_saved_once_it_is_stable_and_only_once() {
    let mut detector = SlideChangeDetector::new();
    let title = slide(&[(100, 160)]);

    assert_eq!(
        saved(
            &mut detector,
            &[title.clone(), title.clone(), title.clone(), title]
        ),
        vec![false, true, false, false]
    );
}

/// Verifies a frame that keeps changing (a transition, animation or video) is never saved.
#[test]
fn a_frame_that_keeps_changing_is_never_saved() {
    let mut detector = SlideChangeDetector::new();
    let frames: Vec<ScreenFrame> = (0..6)
        .map(|step| slide(&[(100 + step * 90, 160 + step * 90)]))
        .collect();

    assert!(
        saved(&mut detector, &frames)
            .iter()
            .all(|was_saved| !was_saved)
    );
}

/// Verifies a new line of text on the same slide is a new slide (a bullet build).
#[test]
fn a_new_line_of_text_is_a_new_slide() {
    let mut detector = SlideChangeDetector::new();
    let one_bullet = slide(&[(100, 140)]);
    let two_bullets = slide(&[(100, 140), (200, 240)]);

    assert_eq!(
        saved(
            &mut detector,
            &[
                one_bullet.clone(),
                one_bullet,
                two_bullets.clone(),
                two_bullets
            ]
        ),
        vec![false, true, false, true]
    );
}

/// Verifies a moved mouse cursor and slight noise do not make a new slide.
#[test]
fn a_cursor_and_noise_are_not_a_new_slide() {
    let mut detector = SlideChangeDetector::new();
    let page = slide(&[(100, 160)]);
    let mut with_cursor = page.luma().to_vec();
    for row in 400..424 {
        with_cursor[row * WIDTH + 700..row * WIDTH + 716].fill(0);
    }
    for (pixel_index, value) in with_cursor.iter_mut().enumerate() {
        // Compression noise of up to three levels.
        *value = value.saturating_sub((pixel_index % 4) as u8);
    }
    let with_cursor = frame(with_cursor);

    assert_eq!(
        saved(
            &mut detector,
            &[page.clone(), page, with_cursor.clone(), with_cursor]
        ),
        vec![false, true, false, false]
    );
}

/// Verifies going back to an earlier slide saves it again, since it was shown again.
#[test]
fn returning_to_an_earlier_slide_saves_it_again() {
    let mut detector = SlideChangeDetector::new();
    let first = slide(&[(100, 160)]);
    let second = slide(&[(400, 460)]);

    assert_eq!(
        saved(
            &mut detector,
            &[
                first.clone(),
                first.clone(),
                second.clone(),
                second,
                first.clone(),
                first
            ]
        ),
        vec![false, true, false, true, false, true]
    );
}

/// Verifies `reset` forgets both the previous sample and the last slide, as for a new share.
#[test]
fn reset_starts_over() {
    let mut detector = SlideChangeDetector::new();
    let page = slide(&[(100, 160)]);
    saved(&mut detector, &[page.clone(), page.clone()]);

    detector.reset();

    assert_eq!(
        saved(&mut detector, &[page.clone(), page]),
        vec![false, true]
    );
}

/// Verifies frames of different sizes are compared by their grid, and tiny frames work.
#[test]
fn frames_of_any_size_are_compared() {
    let mut detector = SlideChangeDetector::new();
    let tiny = ScreenFrame::new(4, 3, vec![10; 12], b"\x89PNG".to_vec())
        .unwrap_or_else(|error| panic!("tiny frame should be valid: {error}"));
    let tiny_white = ScreenFrame::new(4, 3, vec![250; 12], b"\x89PNG".to_vec())
        .unwrap_or_else(|error| panic!("tiny frame should be valid: {error}"));

    assert_eq!(
        saved(
            &mut detector,
            &[tiny.clone(), tiny, tiny_white.clone(), tiny_white]
        ),
        vec![false, true, false, true]
    );
}

/// Verifies the frame contract rejects empty or mismatched frames.
#[test]
fn invalid_frames_are_rejected() {
    assert!(ScreenFrame::new(0, 10, Vec::new(), Vec::new()).is_err());
    assert!(ScreenFrame::new(4, 4, vec![0; 15], Vec::new()).is_err());
}
