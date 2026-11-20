//! Specifies projects (Issue #85): valid names, and the rule that makes every paired device agree
//! on a project's name whatever order changes arrive in.

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use super::projects::{
    Project, ProjectId, ProjectName, ProjectNameError, is_within, shown_parents,
};

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

/// A project numbered `number`, placed under `parent` (by number) at `moved_seconds`.
fn placed(number: u128, parent: Option<u128>, moved_seconds: i64) -> Project {
    Project::new(
        ProjectId::from_uuid(Uuid::from_u128(number)),
        ProjectName::parse(&format!("P{number}")).unwrap_or_else(|error| panic!("{error}")),
        at(0),
        at(0),
    )
    .with_placement(
        parent.map(|parent| ProjectId::from_uuid(Uuid::from_u128(parent))),
        at(moved_seconds),
    )
}

/// The ID numbered `number`.
fn id(number: u128) -> ProjectId {
    ProjectId::from_uuid(Uuid::from_u128(number))
}

#[test]
fn a_new_project_is_at_the_top_level_placed_when_created() {
    let project = project("Roadmap", 5);

    assert_eq!(project.parent(), None);
    assert_eq!(project.moved_at(), project.created_at());
}

#[test]
fn the_newest_move_wins_whatever_the_name_says() {
    let older = placed(1, Some(2), 10);
    let newer = placed(1, None, 20);

    assert!(newer.placement_supersedes(&older));
    assert!(!older.placement_supersedes(&newer));
    assert!(!newer.placement_supersedes(&newer.clone()));
}

#[test]
fn moves_at_the_same_instant_settle_on_the_same_parent_everywhere() {
    let under_two = placed(1, Some(2), 10);
    let under_three = placed(1, Some(3), 10);

    assert_ne!(
        under_two.placement_supersedes(&under_three),
        under_three.placement_supersedes(&under_two)
    );
}

#[test]
fn a_project_shows_under_its_parent_or_at_the_top_when_the_parent_is_gone() {
    let parents = shown_parents(&[
        placed(1, None, 0),
        placed(2, Some(1), 0),
        placed(3, Some(2), 0),
        placed(4, Some(99), 0),
        placed(5, Some(5), 0),
    ]);

    assert_eq!(parents.get(&id(2)), Some(&Some(id(1))));
    assert_eq!(parents.get(&id(3)), Some(&Some(id(2))));
    assert_eq!(
        parents.get(&id(4)),
        Some(&None),
        "unknown or deleted parent"
    );
    assert_eq!(
        parents.get(&id(5)),
        Some(&None),
        "a project is never its own parent"
    );
}

#[test]
fn a_loop_made_by_moves_on_two_devices_breaks_at_its_newest_move() {
    // One device put 1 inside 2 while another put 2 inside 3 and 3 inside 1.
    let projects = [
        placed(1, Some(2), 30),
        placed(2, Some(3), 10),
        placed(3, Some(1), 20),
        placed(4, Some(3), 5),
    ];

    let parents = shown_parents(&projects);
    let mut reversed = projects.to_vec();
    reversed.reverse();

    assert_eq!(
        parents.get(&id(1)),
        Some(&None),
        "the newest move is undone"
    );
    assert_eq!(parents.get(&id(3)), Some(&Some(id(1))));
    assert_eq!(parents.get(&id(2)), Some(&Some(id(3))));
    assert_eq!(parents.get(&id(4)), Some(&Some(id(3))));
    assert_eq!(
        shown_parents(&reversed),
        parents,
        "the same on every device"
    );
}

#[test]
fn a_project_cannot_move_inside_itself_or_its_own_subprojects() {
    let parents = shown_parents(&[
        placed(1, None, 0),
        placed(2, Some(1), 0),
        placed(3, Some(2), 0),
        placed(4, None, 0),
    ]);

    assert!(is_within(&parents, id(3), id(1)));
    assert!(is_within(&parents, id(1), id(1)));
    assert!(!is_within(&parents, id(1), id(3)));
    assert!(!is_within(&parents, id(4), id(1)));
}
