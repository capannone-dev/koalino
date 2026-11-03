//! Specifies projects (Issue #85): valid names, and the rule that makes every paired device agree
//! on a project's name whatever order changes arrive in.

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use super::projects::{Project, ProjectId, ProjectName, ProjectNameError};

/// A fixed instant `seconds` after a reference point.
fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + seconds, 0)
        .single()
        .unwrap_or_else(|| panic!("the instant must exist"))
}

/// A project named `name`, last changed at `updated_seconds`.
fn project(name: &str, updated_seconds: i64) -> Project {
    Project::new(
        ProjectId::from_uuid(Uuid::from_u128(7)),
        ProjectName::parse(name).unwrap_or_else(|error| panic!("{error}")),
        at(0),
        at(updated_seconds),
    )
}

#[test]
fn a_name_is_trimmed() {
    let name =
        ProjectName::parse("  Website relaunch \t").unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(name.as_str(), "Website relaunch");
}

#[test]
fn a_blank_long_or_multi_line_name_is_refused() {
    assert_eq!(ProjectName::parse(" \n "), Err(ProjectNameError::Blank));
    assert_eq!(
        ProjectName::parse(&"x".repeat(ProjectName::MAX_CHARACTERS + 1)),
        Err(ProjectNameError::TooLong {
            max_characters: ProjectName::MAX_CHARACTERS
        })
    );
    assert_eq!(
        ProjectName::parse("Plan\nB"),
        Err(ProjectNameError::ControlCharacter)
    );
    assert!(ProjectName::parse(&"é".repeat(ProjectName::MAX_CHARACTERS)).is_ok());
}

#[test]
fn a_project_id_round_trips_through_text_and_rejects_other_text() {
    let project_id = ProjectId::new_random();

    assert_eq!(ProjectId::parse(&project_id.to_string()), Some(project_id));
    assert_eq!(ProjectId::parse("not-a-project"), None);
}

#[test]
fn the_newest_name_wins() {
    let older = project("Old name", 10);
    let newer = project("New name", 20);

    assert!(newer.supersedes(&older));
    assert!(!older.supersedes(&newer));
}

#[test]
fn at_the_same_instant_the_greater_name_wins_on_every_device() {
    let first = project("Alpha", 10);
    let second = project("Beta", 10);

    assert!(second.supersedes(&first));
    assert!(!first.supersedes(&second));
    assert!(!first.supersedes(&first.clone()));
}
