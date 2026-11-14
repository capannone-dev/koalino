//! Searches meetings and projects for the owner's words (#100).
//!
//! Text is sealed at rest (#92), so SQL can't match it: search reads each meeting and file
//! through the repository, which unseals it, and matches in memory. Nothing it reads is cached or
//! written anywhere. Matching ignores case and accents (`fold`) and needs every word of the query
//! in the same passage: a transcript segment, or one line of a title, summary, notes or file.
//!
//! Scopes:
//! - `Meetings`: every listed meeting (folded recordings are left to the meeting they joined):
//!   its title, summary, notes and transcript.
//! - `Projects`: every project's name, files (names and text), and meetings.
//! - `Project(id)`: one project, the same way.
//!
//! Each hit names where it is (meeting and time, or project and file) and carries a snippet of
//! at most about 180 characters, split into parts so a viewer can mark the matched words without
//! counting characters. At most `MAX_HITS` hits are returned; `truncated` says there were more.

use std::cmp::Reverse;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{
    combined_meetings::MeetingListing, export_manager::SpeakerLabels, meeting_edits::read_notes,
    meeting_summary_document::read_summary,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{Attachment, MeetingId},
        projects::ProjectId,
    },
    ports::{projects::ProjectRepository, storage::NoteRepository},
};

/// The most hits one search returns.
pub const MAX_HITS: usize = 200;

/// The shortest query accepted, in characters (without spaces).
const MIN_QUERY_CHARS: usize = 2;

/// The longest query accepted, in characters.
const MAX_QUERY_CHARS: usize = 200;

/// Characters of context kept before the first match in a long passage.
const CONTEXT_BEFORE: usize = 60;

/// The longest snippet, in characters, before the ellipses.
const SNIPPET_CHARS: usize = 180;

/// Why a search was refused or failed.
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// The query has no words.
    #[error("the query is empty")]
    Empty,
    /// The query is too short to be useful.
    #[error("the query is shorter than {MIN_QUERY_CHARS} characters")]
    TooShort,
    /// The query is too long.
    #[error("the query is longer than {MAX_QUERY_CHARS} characters")]
    TooLong,
    /// The project to search does not exist.
    #[error("there is no such project")]
    ProjectNotFound,
    /// The database could not be read.
    #[error("the notes could not be read: {0}")]
    Storage(#[from] StorageError),
}

/// A parsed query: its words, folded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    terms: Vec<String>,
}

impl SearchQuery {
    /// Parses what the owner typed.
    ///
    /// Inputs: The query text. Outputs: The query, or `Empty`, `TooShort` (fewer than two
    /// characters besides spaces) or `TooLong` (over 200 characters). Side effects: None.
    pub fn parse(text: &str) -> Result<Self, SearchError> {
        if text.chars().count() > MAX_QUERY_CHARS {
            return Err(SearchError::TooLong);
        }
        let folded = fold(text);
        let mut terms: Vec<String> = Vec::new();
        for word in folded.split_whitespace() {
            if !terms.iter().any(|term| term == word) {
                terms.push(word.to_owned());
            }
        }
        if terms.is_empty() {
            return Err(SearchError::Empty);
        }
        if terms.iter().map(|term| term.chars().count()).sum::<usize>() < MIN_QUERY_CHARS {
            return Err(SearchError::TooShort);
        }
        Ok(Self { terms })
    }

    /// The folded words, in the order typed, without repeats.
    pub fn terms(&self) -> &[String] {
        &self.terms
    }
}

/// What to search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchScope {
    /// Every meeting.
    Meetings,
    /// Every project, with its files and meetings.
    Projects,
    /// One project, with its files and meetings.
    Project(ProjectId),
}

/// Where in a meeting or project a hit is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchPlace {
    /// The meeting's title.
    MeetingTitle,
    /// A line of the meeting's summary.
    Summary,
    /// A line of the owner's notes.
    Notes,
    /// A transcript segment.
    Transcript {
        start_time_ms: u64,
        end_time_ms: u64,
        /// The speaker's name, "Speaker N" for an unnamed voice (#118) or `Unattributed`.
        speaker: String,
    },
    /// A line of a transcript imported without times.
    TranscriptText,
    /// The project's name.
    ProjectName,
    /// A file's name.
    ResourceName,
    /// A line of a file's text.
    ResourceText,
}

/// The meeting a hit is in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingRef {
    pub meeting_id: MeetingId,
    /// The title as shown, the fallback for an untitled meeting.
    pub title: String,
    pub created_at: DateTime<Utc>,
}

/// The project a hit is in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRef {
    pub project_id: ProjectId,
    pub name: String,
}

/// The file a hit is in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRef {
    pub attachment_id: Uuid,
    /// The name shown for the file.
    pub file_name: String,
    pub mime_type: String,
}

/// A piece of a snippet, matched or not.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnippetPart {
    pub text: String,
    /// Whether this piece is a matched word.
    pub matched: bool,
}

/// One match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchHit {
    pub place: SearchPlace,
    /// The meeting, unless the hit is in a project's name or file.
    pub meeting: Option<MeetingRef>,
    /// The project, when searching projects.
    pub project: Option<ProjectRef>,
    /// The file, for a hit in a file.
    pub resource: Option<ResourceRef>,
    /// The matched passage, cut around the first match when long.
    pub snippet: Vec<SnippetPart>,
}

/// The hits of one search.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    /// Whether more hits were left out after `MAX_HITS`.
    pub truncated: bool,
}

/// What search needs from its host besides the repository.
pub trait SearchSources {
    /// The name of a saved person, if any.
    fn speaker_name(&self, identity_id: Uuid) -> Option<String>;

    /// A file's searchable text (plain text, Markdown, an e-mail's headers and body), or `None`
    /// for a file without text or one not on this device yet.
    fn resource_text(&self, resource: &Attachment) -> Option<String>;
}

/// Lowercases `text` and removes accents from Latin letters, so `Perché` matches `perche`.
///
/// Outputs: The folded text. Side effects: None.
pub fn fold(text: &str) -> String {
    folded_chars(text).0.into_iter().collect()
}

/// Folds `text` character by character.
///
/// Outputs: The folded characters and, for each, the index of the original character it came
/// from (one original character may give several, as `ß` gives `ss`). Side effects: None.
fn folded_chars(text: &str) -> (Vec<char>, Vec<usize>) {
    let mut folded = Vec::with_capacity(text.len());
    let mut origins = Vec::with_capacity(text.len());
    for (index, original) in text.chars().enumerate() {
        for lower in original.to_lowercase() {
            match strip_accent(lower) {
                Some(base) => {
                    for character in base.chars() {
                        folded.push(character);
                        origins.push(index);
                    }
                }
                None => {
                    folded.push(lower);
                    origins.push(index);
                }
            }
        }
    }
    (folded, origins)
}

/// The base letters of an accented lowercase Latin letter, or `None` for any other character.
fn strip_accent(letter: char) -> Option<&'static str> {
    Some(match letter {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => "s",
        'ţ' | 'ť' | 'ŧ' | 'ț' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'ß' => "ss",
        'æ' => "ae",
        'œ' => "oe",
        _ => return None,
    })
}

/// Matches `text` (one passage) against the query.
///
/// Outputs: The snippet when every word occurs, otherwise `None`. Side effects: None.
fn match_passage(query: &SearchQuery, text: &str) -> Option<Vec<SnippetPart>> {
    let original: Vec<char> = text.chars().collect();
    let (folded, origins) = folded_chars(text);
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for term in &query.terms {
        let needle: Vec<char> = term.chars().collect();
        let mut found = false;
        if needle.len() <= folded.len() {
            for start in 0..=(folded.len() - needle.len()) {
                if folded[start..start + needle.len()] == needle[..] {
                    found = true;
                    let first = origins[start];
                    let last = origins[start + needle.len() - 1];
                    ranges.push((first, last + 1));
                }
            }
        }
        if !found {
            return None;
        }
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    Some(snippet(&original, &merged))
}

/// Cuts a long passage around its first match and splits it into marked and unmarked parts.
fn snippet(original: &[char], matches: &[(usize, usize)]) -> Vec<SnippetPart> {
    let first = matches.first().map_or(0, |range| range.0);
    let (begin, end) = if original.len() <= SNIPPET_CHARS {
        (0, original.len())
    } else {
        let begin = first.saturating_sub(CONTEXT_BEFORE);
        let end = (begin + SNIPPET_CHARS).min(original.len());
        (end.saturating_sub(SNIPPET_CHARS).min(begin), end)
    };
    let mut parts: Vec<SnippetPart> = Vec::new();
    let mut push = |text: String, matched: bool| {
        if !text.is_empty() {
            parts.push(SnippetPart { text, matched });
        }
    };
    let mut cursor = begin;
    let mut prefix = if begin > 0 {
        String::from("…")
    } else {
        String::new()
    };
    for &(start, stop) in matches {
        let (start, stop) = (start.max(begin), stop.min(end));
        if start >= stop {
            continue;
        }
        prefix.extend(&original[cursor..start]);
        push(std::mem::take(&mut prefix), false);
        push(original[start..stop].iter().collect(), true);
        cursor = stop;
    }
    prefix.extend(&original[cursor..end]);
    if end < original.len() {
        prefix.push('…');
    }
    push(prefix, false);
    parts
}

/// Collects hits up to `MAX_HITS`.
struct Collector {
    results: SearchResults,
}

impl Collector {
    /// Whether no more hits fit.
    fn is_full(&self) -> bool {
        self.results.truncated
    }

    /// Adds a hit; past the limit, marks the results as truncated instead.
    fn push(&mut self, hit: SearchHit) {
        if self.results.hits.len() >= MAX_HITS {
            self.results.truncated = true;
        } else {
            self.results.hits.push(hit);
        }
    }
}

/// The non-empty, trimmed lines of a text.
fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty())
}

/// Searches meetings or projects.
///
/// Inputs: The repository, the host's speaker names and file reader, the parsed query and the
/// scope. Outputs: The hits in order: meetings newest first, each its title, summary, notes and
/// transcript in time order; projects by name, each its name, its files (name, then text) as
/// added, then its meetings. `ProjectNotFound` for an unknown project; a storage error.
/// Side effects: Reads (and so unseals) meetings and project files; nothing is written or kept.
pub fn search<Repository>(
    repository: &Repository,
    sources: &dyn SearchSources,
    query: &SearchQuery,
    scope: &SearchScope,
) -> Result<SearchResults, SearchError>
where
    Repository: NoteRepository + ProjectRepository + ?Sized,
{
    let listing = MeetingListing::of(&repository.meeting_recordings()?);
    let mut collector = Collector {
        results: SearchResults::default(),
    };
    match scope {
        SearchScope::Meetings => {
            let meeting_ids: Vec<MeetingId> = repository
                .list_meeting_ids()?
                .into_iter()
                .filter(|meeting_id| listing.is_listed(meeting_id))
                .collect();
            search_meetings(
                repository,
                sources,
                query,
                &meeting_ids,
                None,
                &mut collector,
            )?;
        }
        SearchScope::Projects => {
            for summary in repository.list_projects()? {
                if collector.is_full() {
                    break;
                }
                search_project(
                    repository,
                    sources,
                    query,
                    &listing,
                    summary.project.project_id(),
                    summary.project.name().as_str(),
                    &mut collector,
                )?;
            }
        }
        SearchScope::Project(project_id) => {
            let project = repository
                .load_project(*project_id)?
                .ok_or(SearchError::ProjectNotFound)?;
            search_project(
                repository,
                sources,
                query,
                &listing,
                *project_id,
                project.name().as_str(),
                &mut collector,
            )?;
        }
    }
    Ok(collector.results)
}

/// Searches one project's name, files and meetings.
fn search_project<Repository>(
    repository: &Repository,
    sources: &dyn SearchSources,
    query: &SearchQuery,
    listing: &MeetingListing,
    project_id: ProjectId,
    name: &str,
    collector: &mut Collector,
) -> Result<(), SearchError>
where
    Repository: NoteRepository + ProjectRepository + ?Sized,
{
    let project = ProjectRef {
        project_id,
        name: name.to_owned(),
    };
    let hit = |place: SearchPlace, resource: Option<ResourceRef>, snippet| SearchHit {
        place,
        meeting: None,
        project: Some(project.clone()),
        resource,
        snippet,
    };
    if let Some(snippet) = match_passage(query, name) {
        collector.push(hit(SearchPlace::ProjectName, None, snippet));
    }
    let mut resources = repository.project_resources(project_id)?;
    resources.sort_by_key(Attachment::original_timestamp);
    for resource in resources {
        if collector.is_full() {
            return Ok(());
        }
        let reference = ResourceRef {
            attachment_id: resource.attachment_id(),
            file_name: resource.shown_file_name(),
            mime_type: resource.mime_type().to_owned(),
        };
        if let Some(snippet) = match_passage(query, &reference.file_name) {
            collector.push(hit(
                SearchPlace::ResourceName,
                Some(reference.clone()),
                snippet,
            ));
        }
        if let Some(text) = sources.resource_text(&resource) {
            for line in lines(&text) {
                if let Some(snippet) = match_passage(query, line) {
                    collector.push(hit(
                        SearchPlace::ResourceText,
                        Some(reference.clone()),
                        snippet,
                    ));
                }
            }
        }
    }
    let meeting_ids: Vec<MeetingId> = repository
        .project_meeting_ids(project_id)?
        .into_iter()
        .filter(|meeting_id| listing.is_listed(meeting_id))
        .collect();
    search_meetings(
        repository,
        sources,
        query,
        &meeting_ids,
        Some(&project),
        collector,
    )
}

/// Searches meetings, newest first.
fn search_meetings<Repository>(
    repository: &Repository,
    sources: &dyn SearchSources,
    query: &SearchQuery,
    meeting_ids: &[MeetingId],
    project: Option<&ProjectRef>,
    collector: &mut Collector,
) -> Result<(), SearchError>
where
    Repository: NoteRepository + ?Sized,
{
    let mut meetings = Vec::with_capacity(meeting_ids.len());
    for meeting_id in meeting_ids {
        if let Some(metadata) = repository.load_meeting_metadata(meeting_id)? {
            meetings.push(MeetingRef {
                meeting_id: meeting_id.clone(),
                title: metadata.display_title(),
                created_at: metadata.created_at(),
            });
        }
    }
    meetings.sort_by_key(|meeting| Reverse(meeting.created_at));
    for meeting in meetings {
        if collector.is_full() {
            break;
        }
        search_meeting(repository, sources, query, &meeting, project, collector)?;
    }
    Ok(())
}

/// Searches one meeting's title, summary, notes and transcript.
fn search_meeting<Repository>(
    repository: &Repository,
    sources: &dyn SearchSources,
    query: &SearchQuery,
    meeting: &MeetingRef,
    project: Option<&ProjectRef>,
    collector: &mut Collector,
) -> Result<(), SearchError>
where
    Repository: NoteRepository + ?Sized,
{
    let push = |collector: &mut Collector, place: SearchPlace, snippet| {
        collector.push(SearchHit {
            place,
            meeting: Some(meeting.clone()),
            project: project.cloned(),
            resource: None,
            snippet,
        });
    };
    if let Some(snippet) = match_passage(query, &meeting.title) {
        push(collector, SearchPlace::MeetingTitle, snippet);
    }
    let document = repository.load_crdt_document(&meeting.meeting_id)?;
    // A document that can't be read has no summary or notes to find; the transcript still can.
    if let Ok(Some(summary)) = read_summary(&document) {
        for line in lines(&summary.text) {
            if let Some(snippet) = match_passage(query, line) {
                push(collector, SearchPlace::Summary, snippet);
            }
        }
    }
    if let Ok(Some(notes)) = read_notes(&document) {
        for line in lines(&notes) {
            if let Some(snippet) = match_passage(query, line) {
                push(collector, SearchPlace::Notes, snippet);
            }
        }
    }
    let mut segments = repository.load_transcript_segments(&meeting.meeting_id)?;
    if segments.is_empty() {
        if let Some(transcript) = repository.load_transcript(&meeting.meeting_id)? {
            for line in lines(transcript.content()) {
                if let Some(snippet) = match_passage(query, line) {
                    push(collector, SearchPlace::TranscriptText, snippet);
                }
            }
        }
        return Ok(());
    }
    segments.sort_by_key(|segment| (segment.start_time_ms(), segment.end_time_ms()));
    // Saved names decide which voices are numbered, so the numbers match the transcript (#118).
    let names: std::collections::HashMap<Uuid, String> = segments
        .iter()
        .filter_map(|segment| segment.speaker_identity_id())
        .filter_map(|identity_id| Some((identity_id, sources.speaker_name(identity_id)?)))
        .collect();
    let speaker_labels = SpeakerLabels::for_segments(&segments, |identity_id| {
        names.get(&identity_id).map(String::as_str)
    });
    for segment in &segments {
        if collector.is_full() {
            return Ok(());
        }
        let Some(snippet) = match_passage(query, segment.text().trim()) else {
            continue;
        };
        let speaker = speaker_labels.label(segment).to_owned();
        push(
            collector,
            SearchPlace::Transcript {
                start_time_ms: segment.start_time_ms(),
                end_time_ms: segment.end_time_ms(),
                speaker,
            },
            snippet,
        );
    }
    Ok(())
}
