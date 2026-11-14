//! Specifies how a chat answer's citations ([n] and [n, hh:mm:ss]) become links (#110).

use super::chat::{ChatSource, ChatTarget, CitedMaterial, parse_citations};

fn material() -> Vec<CitedMaterial> {
    vec![
        CitedMaterial {
            label: String::from("Meeting \"Budget\", 2026-11-09 10:00 UTC"),
            target: Some(ChatTarget::Meeting {
                meeting_id: String::from("m-1"),
            }),
        },
        CitedMaterial {
            label: String::from("File plan.md"),
            target: Some(ChatTarget::File {
                project_id: Some(String::from("p-1")),
                resource_id: String::from("r-1"),
            }),
        },
    ]
}

#[test]
fn numbers_and_times_become_sources_in_order_of_first_mention() {
    let sources = parse_citations(
        "The budget was cut [1, 00:12:34] and the plan says so [2]. Again [1, 00:12:34] [1].",
        &material(),
    );
    assert_eq!(
        sources,
        vec![
            ChatSource {
                number: 1,
                label: String::from("Meeting \"Budget\", 2026-11-09 10:00 UTC"),
                target: Some(ChatTarget::Meeting {
                    meeting_id: String::from("m-1"),
                }),
                at_ms: Some(754_000),
            },
            ChatSource {
                number: 2,
                label: String::from("File plan.md"),
                target: Some(ChatTarget::File {
                    project_id: Some(String::from("p-1")),
                    resource_id: String::from("r-1"),
                }),
                at_ms: None,
            },
        ],
        "the same place cited twice is listed once, and the bare [1] adds nothing to its moment"
    );
}

#[test]
fn lists_short_times_and_unknown_numbers_are_handled() {
    let sources = parse_citations(
        "See [1, 2] and [2, 12:34] but not [3], [0] or [x].",
        &material(),
    );
    let found: Vec<(u16, Option<u64>)> = sources
        .iter()
        .map(|source| (source.number, source.at_ms))
        .collect();
    assert_eq!(found, vec![(1, None), (2, Some(754_000))]);
}

#[test]
fn markdown_links_and_checkboxes_are_not_citations() {
    let sources = parse_citations("- [ ] call [the bank](https://x.y) [ 1 ]", &material());
    assert_eq!(sources.len(), 1, "spaces inside the brackets are fine");
    assert!(parse_citations("[1](https://x.y)", &material()).is_empty());
}

#[test]
fn a_meeting_cited_at_moments_is_not_listed_again_without_one() {
    let sources = parse_citations(
        "We approved it [1]. The budget came later [1, 00:00:40], then the date [1, 00:01:15]. \
         See the notes [2].",
        &material(),
    );

    let places: Vec<(u16, Option<u64>)> = sources
        .iter()
        .map(|source| (source.number, source.at_ms))
        .collect();
    assert_eq!(
        places,
        vec![(1, Some(40_000)), (1, Some(75_000)), (2, None)]
    );
}
