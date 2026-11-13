//! Specifies the reading budget and how labels become distinct names (#113).

use std::time::Duration;

use super::{ReadBudget, SpeakingNames};

#[test]
fn the_budget_stops_after_its_elements() {
    let mut budget = ReadBudget::new(2, Duration::from_secs(10));
    assert!(budget.visit());
    assert!(budget.visit());
    assert!(!budget.visit());
}

#[test]
fn the_budget_stops_after_its_time() {
    let mut budget = ReadBudget::new(10, Duration::ZERO);
    assert!(!budget.visit());
}

#[test]
fn labels_give_each_speaking_name_once() {
    let mut names = SpeakingNames::default();
    for label in [
        "",
        "Alice is speaking",
        "Mute",
        "Alice is speaking",
        "Talking: Bruno",
    ] {
        names.read_label(label);
    }
    assert_eq!(names.into_names(), vec!["Alice", "Bruno"]);
}
