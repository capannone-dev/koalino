//! Specifies search (#100): case- and accent-insensitive matching of every word, over meeting
//! titles, summaries, notes and transcripts, and over projects' names, files and meetings, with
//! snippets that mark the matches and say where each one is.

use std::{collections::HashMap, sync::Arc};

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::{
    meeting_crdt::MeetingCrdtService,
    meeting_document::AutomergeMeetingDocumentCodec,
    meeting_edits::{edit_summary, set_notes},
    search::{
        MAX_HITS, SearchError, SearchPlace, SearchQuery, SearchScope, SearchSources, SnippetPart,
        fold, search,
    },
};
use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::crypto::voice_encryptor::ephemeral_cipher,
    domain::{
        models::{Attachment, MeetingId, MeetingTitle, TranscriptSegment},
        projects::{ProjectId, ProjectName},
    },
    ports::{
        projects::ProjectRepository,
        storage::{AttachmentRepository, NoteRepository},
    },
};

/// An in-memory repository with the meeting-document codec.
fn repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"))
}

/// Stores a titled meeting.
fn meeting(repository: &SqliteNoteRepository, id: &str, title: &str) -> MeetingId {
    let meeting_id = MeetingId::new(id);
    let title = MeetingTitle::parse(title).unwrap_or_else(|e| panic!("{e}"));
    MeetingCrdtService::new(repository)
        .create_titled_meeting(&meeting_id, &title)
        .unwrap_or_else(|e| panic!("{e}"));
    meeting_id
}

/// Stores the segments `(start ms, speaker, text)` of a meeting.
fn segments(
    repository: &SqliteNoteRepository,
    meeting_id: &MeetingId,
    lines: &[(u64, Option<Uuid>, &str)],
) {
    let segments: Vec<TranscriptSegment> = lines
        .iter()
        .map(|(start, speaker, text)| {
            TranscriptSegment::new(meeting_id.clone(), *start, start + 1_000, *speaker, *text)
        })
        .collect();
    repository
        .save_transcript_segments(meeting_id, &segments)
        .unwrap_or_else(|e| panic!("{e}"));
}

/// Speaker names and file texts for the tests.
#[derive(Default)]
struct Sources {
    names: HashMap<Uuid, String>,
    texts: HashMap<Uuid, String>,
}

impl SearchSources for Sources {
    fn speaker_name(&self, identity_id: Uuid) -> Option<String> {
        self.names.get(&identity_id).cloned()
    }

    fn resource_text(&self, resource: &Attachment) -> Option<String> {
        self.texts.get(&resource.attachment_id()).cloned()
    }
}

/// The snippet's marked text.
fn marked(parts: &[SnippetPart]) -> Vec<String> {
    parts
        .iter()
        .filter(|part| part.matched)
        .map(|part| part.text.clone())
        .collect()
}

/// The snippet as one string.
fn whole(parts: &[SnippetPart]) -> String {
    parts.iter().map(|part| part.text.as_str()).collect()
}

fn query(text: &str) -> SearchQuery {
    SearchQuery::parse(text).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn folding_ignores_case_and_accents() {
    assert_eq!(fold("Perché È ÇA Straße"), "perche e ca strasse");
    assert_eq!(fold("Ünïcödé ñ"), "unicode n");
}

#[test]
fn queries_must_be_meaningful() {
    assert!(matches!(SearchQuery::parse("   "), Err(SearchError::Empty)));
    assert!(matches!(
        SearchQuery::parse("a"),
        Err(SearchError::TooShort)
    ));
    assert!(matches!(
        SearchQuery::parse(&"x".repeat(201)),
        Err(SearchError::TooLong)
    ));
    assert_eq!(query("  Budget   Q3 ").terms(), ["budget", "q3"]);
}

#[test]
fn a_transcript_hit_says_who_spoke_when_and_marks_every_word() {
    let repository = repository();
    let anna = Uuid::new_v4();
    let budget = meeting(&repository, "m-1", "Weekly sync");
    segments(
        &repository,
        &budget,
        &[
            (0, Some(anna), "Good morning everyone."),
            (
                65_000,
                Some(anna),
                "Il budget è approvato, perché il Q3 va bene.",
            ),
            (90_000, None, "Approved budget? Yes."),
        ],
    );
    let sources = Sources {
        names: HashMap::from([(anna, String::from("Anna"))]),
        ..Sources::default()
    };

    let results = search(
        &repository,
        &sources,
        &query("APPROVATO budget"),
        &SearchScope::Meetings,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(results.hits.len(), 1, "every word must be in the passage");
    let hit = &results.hits[0];
    assert_eq!(
        hit.meeting.as_ref().map(|m| m.title.as_str()),
        Some("Weekly sync")
    );
    assert_eq!(
        hit.place,
        SearchPlace::Transcript {
            start_time_ms: 65_000,
            end_time_ms: 66_000,
            speaker: String::from("Anna"),
        }
    );
    assert_eq!(marked(&hit.snippet), ["budget", "approvato"]);
    assert_eq!(
        whole(&hit.snippet),
        "Il budget è approvato, perché il Q3 va bene."
    );
    assert!(!results.truncated);

    let accents = search(
        &repository,
        &sources,
        &query("perche"),
        &SearchScope::Meetings,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(marked(&accents.hits[0].snippet), ["perché"]);
}

#[test]
fn titles_summaries_and_notes_are_searched_before_the_transcript() {
    let repository = repository();
    let meeting_id = meeting(&repository, "m-1", "Courier contract");
    set_notes(
        &repository,
        &meeting_id,
        "# Actions\n\n- Call the courier on Monday",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    edit_summary(
        &repository,
        &meeting_id,
        "The courier will be replaced.",
        Utc::now(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    segments(&repository, &meeting_id, &[(5_000, None, "Which courier?")]);

    let results = search(
        &repository,
        &Sources::default(),
        &query("courier"),
        &SearchScope::Meetings,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let places: Vec<&SearchPlace> = results.hits.iter().map(|hit| &hit.place).collect();
    assert_eq!(places.len(), 4);
    assert_eq!(places[0], &SearchPlace::MeetingTitle);
    assert_eq!(places[1], &SearchPlace::Summary);
    assert_eq!(places[2], &SearchPlace::Notes);
    assert!(matches!(
        places[3],
        SearchPlace::Transcript {
            start_time_ms: 5_000,
            ..
        }
    ));
    assert_eq!(
        whole(&results.hits[2].snippet),
        "- Call the courier on Monday"
    );
}

#[test]
fn a_long_passage_is_cut_around_the_match() {
    let repository = repository();
    let meeting_id = meeting(&repository, "m-1", "Long");
    let long = format!("{} needle {}", "word ".repeat(80), "tail ".repeat(80));
    segments(&repository, &meeting_id, &[(0, None, &long)]);

    let results = search(
        &repository,
        &Sources::default(),
        &query("needle"),
        &SearchScope::Meetings,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let text = whole(&results.hits[0].snippet);
    assert!(text.starts_with('…') && text.ends_with('…'), "{text}");
    assert!(text.chars().count() < 220, "{}", text.chars().count());
    assert_eq!(marked(&results.hits[0].snippet), ["needle"]);
}

#[test]
fn projects_search_their_names_files_and_meetings() {
    let repository = repository();
    let launch = repository
        .create_project(&ProjectName::parse("Launch").unwrap_or_else(|e| panic!("{e}")))
        .unwrap_or_else(|e| panic!("{e}"));
    let other = repository
        .create_project(&ProjectName::parse("Hiring").unwrap_or_else(|e| panic!("{e}")))
        .unwrap_or_else(|e| panic!("{e}"));
    let plan = resource(&repository, launch.project_id(), "launch-plan.md");
    let cv = resource(&repository, other.project_id(), "cv.txt");
    let inside = meeting(&repository, "m-in", "Kick-off");
    segments(
        &repository,
        &inside,
        &[(1_000, None, "The launch date is set.")],
    );
    repository
        .move_meeting_to_project(&inside, Some(launch.project_id()))
        .unwrap_or_else(|e| panic!("{e}"));
    let outside = meeting(&repository, "m-out", "Unrelated");
    segments(
        &repository,
        &outside,
        &[(1_000, None, "A launch elsewhere.")],
    );
    let sources = Sources {
        texts: HashMap::from([
            (
                plan.attachment_id(),
                String::from("Intro\n\nThe launch is in May."),
            ),
            (cv.attachment_id(), String::from("Led a product launch.")),
        ]),
        ..Sources::default()
    };

    let one = search(
        &repository,
        &sources,
        &query("launch"),
        &SearchScope::Project(launch.project_id()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let places: Vec<&SearchPlace> = one.hits.iter().map(|hit| &hit.place).collect();
    assert_eq!(
        places,
        [
            &SearchPlace::ProjectName,
            &SearchPlace::ResourceName,
            &SearchPlace::ResourceText,
            &SearchPlace::Transcript {
                start_time_ms: 1_000,
                end_time_ms: 2_000,
                speaker: String::from("Unattributed"),
            },
        ]
    );
    assert!(one.hits.iter().all(|hit| {
        hit.project.as_ref().map(|project| project.project_id) == Some(launch.project_id())
    }));
    assert_eq!(
        one.hits[2].resource.as_ref().map(|r| r.file_name.as_str()),
        Some("launch-plan.md")
    );
    assert_eq!(whole(&one.hits[2].snippet), "The launch is in May.");

    let all = search(
        &repository,
        &sources,
        &query("launch"),
        &SearchScope::Projects,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        all.hits.len(),
        5,
        "the meeting outside any project is left out"
    );
    assert!(
        all.hits
            .iter()
            .any(|hit| hit.project.as_ref().map(|p| p.project_id) == Some(other.project_id()))
    );

    let meetings = search(
        &repository,
        &sources,
        &query("launch"),
        &SearchScope::Meetings,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(meetings.hits.len(), 2, "every meeting, but no project file");
}

#[test]
fn results_stop_at_the_limit() {
    let repository = repository();
    let meeting_id = meeting(&repository, "m-1", "Many");
    let lines: Vec<(u64, Option<Uuid>, &str)> = (0..(MAX_HITS as u64 + 10))
        .map(|index| (index * 1_000, None, "same words again"))
        .collect();
    segments(&repository, &meeting_id, &lines);

    let results = search(
        &repository,
        &Sources::default(),
        &query("words"),
        &SearchScope::Meetings,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(results.hits.len(), MAX_HITS);
    assert!(results.truncated);
}

/// Stores a resource row named `file_name` in `project_id` (its text comes from `Sources`).
fn resource(
    repository: &SqliteNoteRepository,
    project_id: ProjectId,
    file_name: &str,
) -> Attachment {
    let attachment_id = Uuid::new_v4();
    let attachment = Attachment::new(
        attachment_id,
        None,
        format!("/data/attachments/{attachment_id}-{file_name}"),
        "text/markdown",
        Utc.with_ymd_and_hms(2026, 11, 9, 9, 0, 0)
            .single()
            .unwrap_or_default(),
        "device",
    )
    .with_project_id(project_id);
    repository
        .save_attachment(&attachment)
        .unwrap_or_else(|e| panic!("{e}"));
    attachment
}
