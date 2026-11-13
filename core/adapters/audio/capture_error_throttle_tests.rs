//! Specifies how often a capture stream's repeated errors are logged.

use std::time::{Duration, Instant};

use super::capture_error_throttle::CaptureErrorThrottle;

#[test]
fn the_first_error_is_logged_then_at_most_one_a_minute_with_the_count_between() {
    let start = Instant::now();
    let mut throttle = CaptureErrorThrottle::default();
    assert_eq!(throttle.note(start), Some(0));
    for second in 1..60 {
        assert_eq!(throttle.note(start + Duration::from_secs(second)), None);
    }
    // The minute is over: one line covers the 59 errors held back.
    assert_eq!(throttle.note(start + Duration::from_secs(60)), Some(59));
    assert_eq!(throttle.note(start + Duration::from_secs(61)), None);
    assert_eq!(throttle.note(start + Duration::from_secs(125)), Some(1));
}
