//! Lets a user review a meeting's unknown-speaker clusters and assign them to named identities.
//!
//! This is the single use case behind the CLI, HTTP, and FFI cluster commands, so those driving
//! adapters only parse input and format output. Listing delegates to
//! `UnknownSpeakerClusterer::get_clusters_for_meeting` and strips biometric vectors; assignment
//! validates the typed name, resolves it to an identity, and delegates to
//! `UnknownSpeakerClusterer::assign_identity_to_cluster`.
//!
//! The voice library also groups unresolved clusters whose voices are close (#82), across chunks,
//! speakers of one recording, and meetings, so one person is named once: naming a group assigns
//! every cluster in it to one identity. Grouping is recomputed on each listing and never stored,
//! so it needs no sync; each cluster still resolves on its own, as paired devices expect.
//!
//! A recording whose speech went into a meeting's combined transcript (#88) is left out: the
//! combined pass gave that meeting its own voices (#155).
//!
//! Each unknown voice may carry a suggested name (#113): the name a call window or the Teams
//! captions showed while it spoke (`domain::speaker_names`). The gallery offers it; nothing is
//! named without the owner's tap.
//!
//! Name resolution: a name matching an identity stored in `VoiceVectorDB` (trimmed,
//! case-insensitive) reuses that identity and keeps its stored spelling; any other name becomes a
//! new identity with a fresh UUID. The clusterer persists the identity, so names survive restarts
//! wherever the composition root uses a persistent voice store (`SqliteVoiceVectorDB`).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    application::{
        export_manager::SpeakerLabels,
        pipeline::meeting_speaker_pass::{average_linkage, dense_labels, unit_vector},
        unknown_speaker_clusterer::{
            ClusterAssignment, ClusteringConfig, ClusteringError, UnknownSpeakerClusterer,
        },
    },
    domain::{
        models::{MeetingId, SpeakerIdentity, UnassignedVoiceClip, UnknownSpeakerCluster},
        speaker_names::{NamedSpan, VoiceTurn, caption_spans, parse_voice_cues, suggest_names},
    },
    ports::{
        storage::{AttachmentRepository, NoteRepository},
        voice::VoiceVectorDB,
    },
};

/// Reports why a cluster could not be listed or assigned.
#[derive(Debug, thiserror::Error)]
pub enum ClusterReviewError {
    /// The typed identity name is empty or whitespace only.
    #[error("identity name must not be blank")]
    BlankIdentityName,
    /// A group naming was asked for without any cluster.
    #[error("at least one voice must be chosen")]
    NoClustersSelected,
    /// The clusterer rejected the request (unknown or resolved cluster, storage failure).
    #[error(transparent)]
    Clustering(#[from] ClusteringError),
}

/// Describes one unresolved cluster without its biometric representative embedding.
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownClusterSummary {
    cluster_id: Uuid,
    meeting_id: MeetingId,
    total_duration_ms: u64,
    chunk_count: usize,
    created_at: DateTime<Utc>,
    /// The label its meeting's transcript shows for this voice, "Speaker N" (#118); `None` when no
    /// segment carries the cluster.
    speaker_label: Option<String>,
}

impl UnknownClusterSummary {
    /// Copies the display-safe fields of `cluster`, dropping its embedding.
    fn from_cluster(cluster: &UnknownSpeakerCluster, labels: &SpeakerLabels) -> Self {
        Self {
            cluster_id: cluster.cluster_id(),
            meeting_id: cluster.meeting_id().clone(),
            total_duration_ms: cluster.total_duration_ms(),
            chunk_count: cluster.chunk_ids().len(),
            created_at: cluster.created_at(),
            speaker_label: labels
                .cluster_label(cluster.cluster_id())
                .map(str::to_owned),
        }
    }

    /// Returns the label the meeting's transcript shows for this voice ("Speaker 2"), so the
    /// review list and the transcript agree (#118).
    pub fn speaker_label(&self) -> Option<&str> {
        self.speaker_label.as_deref()
    }

    /// Returns the cluster UUID.
    pub fn cluster_id(&self) -> Uuid {
        self.cluster_id
    }

    /// Borrows the owning meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the summed speech duration of every member chunk in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_ms
    }

    /// Returns how many voice chunks the cluster contains.
    pub fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    /// Returns when the cluster's first chunk was recorded.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

/// One unresolved cluster as the voice gallery shows it, across all meetings (Issue #63).
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceLibraryCluster {
    summary: UnknownClusterSummary,
    meeting_title: Option<String>,
    has_review_clip: bool,
    /// The group this voice belongs to: the ID of the group's first cluster (#82).
    voice_group_id: Uuid,
    /// The name shown on the call window while this voice spoke (#113), if clear enough.
    suggested_name: Option<String>,
}

impl VoiceLibraryCluster {
    /// Borrows the cluster's display-safe summary.
    pub fn summary(&self) -> &UnknownClusterSummary {
        &self.summary
    }

    /// Returns the owning meeting's title, when it has one.
    pub fn meeting_title(&self) -> Option<&str> {
        self.meeting_title.as_deref()
    }

    /// Reports whether a review clip can still be played for this cluster.
    pub fn has_review_clip(&self) -> bool {
        self.has_review_clip
    }

    /// Returns the name the call window or captions showed while this voice spoke (#113).
    pub fn suggested_name(&self) -> Option<&str> {
        self.suggested_name.as_deref()
    }

    /// Returns the ID of the voice group this cluster belongs to, shared by its whole group (#82).
    pub fn voice_group_id(&self) -> Uuid {
        self.voice_group_id
    }
}

/// Default least average cosine similarity between two clusters' voices for them to be offered as
/// one person (#82).
///
/// Measured on the evaluation recordings (`DECISION_LOG.md`, #82): one person's whole-recording
/// voices in different recordings reach a median of 0.85 (5th percentile 0.58), different people
/// at most 0.41, and 5 s pieces of different people at most 0.49. A wrong merge would name someone
/// silently, while a split costs one more naming, so this errs towards splitting; the owner can also
/// leave a part out.
pub const DEFAULT_VOICE_GROUP_SIMILARITY: f32 = 0.5;

/// Unresolved clusters that sound like one person, offered to be named together (#82).
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownVoiceGroup {
    group_id: Uuid,
    clusters: Vec<VoiceLibraryCluster>,
}

impl UnknownVoiceGroup {
    /// Returns the group's ID, the ID of its first cluster. It changes if that cluster goes.
    pub fn group_id(&self) -> Uuid {
        self.group_id
    }

    /// Borrows the group's clusters, in library order.
    pub fn clusters(&self) -> &[VoiceLibraryCluster] {
        &self.clusters
    }

    /// Returns the speech of every cluster in the group, in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.clusters
            .iter()
            .map(|cluster| cluster.summary.total_duration_ms)
            .sum()
    }

    /// Returns how many different meetings the group's clusters come from.
    pub fn meeting_count(&self) -> usize {
        let mut meetings: Vec<&MeetingId> = Vec::new();
        for cluster in &self.clusters {
            if !meetings.contains(&&cluster.summary.meeting_id) {
                meetings.push(&cluster.summary.meeting_id);
            }
        }
        meetings.len()
    }
}

/// One saved person, with how much voice data is held but none of it exposed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedPersonSummary {
    identity_id: Uuid,
    display_name: String,
    voice_variant_count: usize,
}

impl SavedPersonSummary {
    /// Returns the identity UUID.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// Borrows the stored display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns how many voice variants are stored for this person.
    pub fn voice_variant_count(&self) -> usize {
        self.voice_variant_count
    }
}

/// Everything the voice gallery shows: voices to name and people already saved.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceLibrary {
    unknown_clusters: Vec<VoiceLibraryCluster>,
    unknown_voice_groups: Vec<UnknownVoiceGroup>,
    saved_people: Vec<SavedPersonSummary>,
}

impl VoiceLibrary {
    /// Borrows the unresolved clusters, group by group: groups in the order their first cluster
    /// appears (meeting order, then creation order), and a group's clusters next to each other.
    pub fn unknown_clusters(&self) -> &[VoiceLibraryCluster] {
        &self.unknown_clusters
    }

    /// Borrows the unresolved clusters grouped by voice, in the same order (#82).
    pub fn unknown_voice_groups(&self) -> &[UnknownVoiceGroup] {
        &self.unknown_voice_groups
    }

    /// Borrows the saved people, sorted by name ignoring case.
    pub fn saved_people(&self) -> &[SavedPersonSummary] {
        &self.saved_people
    }
}

/// Summarizes naming several clusters as one person (#82).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedGroupAssignment {
    identity: SpeakerIdentity,
    identity_created: bool,
    assigned_cluster_ids: Vec<Uuid>,
    skipped_cluster_count: usize,
    variants_stored: usize,
    segments_updated: usize,
}

impl NamedGroupAssignment {
    /// Returns the identity every cluster was assigned to.
    pub fn identity_id(&self) -> Uuid {
        self.identity.identity_id()
    }

    /// Returns that identity's stored display name.
    pub fn identity_name(&self) -> &str {
        self.identity.display_name()
    }

    /// Reports whether this naming created the identity's voice-store record.
    pub fn identity_created(&self) -> bool {
        self.identity_created
    }

    /// Borrows the clusters now resolved, in the order given.
    pub fn assigned_cluster_ids(&self) -> &[Uuid] {
        &self.assigned_cluster_ids
    }

    /// Returns how many given clusters were skipped, being already named or gone.
    pub fn skipped_cluster_count(&self) -> usize {
        self.skipped_cluster_count
    }

    /// Returns how many voice variants were stored in all.
    pub fn variants_stored(&self) -> usize {
        self.variants_stored
    }

    /// Returns how many transcript segments were re-attributed in all.
    pub fn segments_updated(&self) -> usize {
        self.segments_updated
    }
}

/// Summarizes a cluster assignment to a named, persisted identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedClusterAssignment {
    assignment: ClusterAssignment,
}

impl NamedClusterAssignment {
    /// Returns the stored display name of the identity the cluster was assigned to.
    pub fn identity_name(&self) -> &str {
        self.assignment.identity().display_name()
    }

    /// Returns the resolved cluster.
    pub fn cluster_id(&self) -> Uuid {
        self.assignment.cluster_id()
    }

    /// Returns the identity the cluster was assigned to.
    pub fn identity_id(&self) -> Uuid {
        self.assignment.identity_id()
    }

    /// Reports whether the identity had no voice variants before this assignment.
    pub fn identity_created(&self) -> bool {
        self.assignment.identity_created()
    }

    /// Returns how many member embeddings were stored as voice variants.
    pub fn variants_stored(&self) -> usize {
        self.assignment.variants_stored()
    }

    /// Returns how many transcript segments were re-attributed to the identity.
    pub fn segments_updated(&self) -> usize {
        self.assignment.segments_updated()
    }
}

/// Lists a meeting's unresolved unknown-speaker clusters for review.
///
/// Inputs: `note_repository` holds clusters (it may be a `dyn` port); `meeting_id` identifies the
/// meeting, and an unknown meeting simply has no clusters.
/// Outputs: Display-safe summaries in creation order, or a storage error.
/// Side effects: Reads cluster rows; the biometric representative vectors are discarded.
pub fn list_unknown_clusters<RepositoryPort, VectorDatabase>(
    note_repository: &RepositoryPort,
    voice_vector_database: &VectorDatabase,
    meeting_id: &MeetingId,
) -> Result<Vec<UnknownClusterSummary>, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    let clusters = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default())
        .get_clusters_for_meeting(meeting_id)?;
    if clusters.is_empty() {
        return Ok(Vec::new());
    }
    let labels = meeting_speaker_labels(note_repository, voice_vector_database, meeting_id)?;
    Ok(clusters
        .iter()
        .map(|cluster| UnknownClusterSummary::from_cluster(cluster, &labels))
        .collect())
}

/// Numbers a meeting's voices as its transcript shows them (#118).
///
/// Outputs: The labels, or a storage error. Side effects: Reads the meeting's segments and the
/// saved people's names.
pub fn meeting_speaker_labels<RepositoryPort, VectorDatabase>(
    note_repository: &RepositoryPort,
    voice_vector_database: &VectorDatabase,
    meeting_id: &MeetingId,
) -> Result<SpeakerLabels, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    let names: std::collections::HashMap<Uuid, String> = voice_vector_database
        .list_speaker_identities()
        .map_err(ClusteringError::from)?
        .into_iter()
        .map(|identity| (identity.identity_id(), identity.display_name().to_owned()))
        .collect();
    let segments = note_repository
        .load_transcript_segments(meeting_id)
        .map_err(ClusteringError::from)?;
    Ok(SpeakerLabels::for_segments(&segments, |identity_id| {
        names.get(&identity_id).map(String::as_str)
    }))
}

/// Gives the names a meeting's call window or captions showed as speaking (#113).
pub trait NamedSpanSource {
    /// Returns the meeting's named spans, in meeting milliseconds; empty when there are none or
    /// they cannot be read. Side effects: May read and decrypt the meeting's attachments.
    fn named_spans(&self, meeting_id: &MeetingId) -> Vec<NamedSpan>;
}

/// Gives no names: the gallery without suggestions.
pub struct NoNamedSpans;

impl NamedSpanSource for NoNamedSpans {
    fn named_spans(&self, _meeting_id: &MeetingId) -> Vec<NamedSpan> {
        Vec::new()
    }
}

/// File-name prefixes of the attachments whose `<v Name>` cues are in recording time: the call
/// window's readings (#113) and the Teams captions and live transcript (#102). The Teams
/// transcript published after the meeting is timed from the Teams meeting's start, so it is not
/// used.
const NAMED_SPAN_FILE_PREFIXES: [&str; 3] = [
    "speaker-names-",
    "teams-captions-",
    "teams-live-transcript-",
];
/// File-name prefix of the call window's readings (#113); the other prefixes are captions.
const CALL_WINDOW_FILE_PREFIX: &str = "speaker-names-";
/// Largest attachment read for names.
const MAX_NAMED_SPAN_FILE_BYTES: u64 = 4 * 1024 * 1024;

/// Reads named spans from a meeting's WebVTT attachments.
pub struct AttachedNamedSpans<'repository, RepositoryPort: ?Sized>(pub &'repository RepositoryPort);

impl<RepositoryPort> NamedSpanSource for AttachedNamedSpans<'_, RepositoryPort>
where
    RepositoryPort: AttachmentRepository + ?Sized,
{
    /// Reads the meeting's `speaker-names`, `teams-captions` and `teams-live-transcript` WebVTT
    /// attachments. Side effects: Decrypts those files in memory; nothing is written or logged.
    fn named_spans(&self, meeting_id: &MeetingId) -> Vec<NamedSpan> {
        let Ok(attachments) = self.0.get_attachments_for_meeting(meeting_id) else {
            return Vec::new();
        };
        let mut spans = Vec::new();
        for attachment in attachments {
            let path = std::path::Path::new(attachment.file_path());
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let is_named_span_file = attachment.mime_type() == "text/vtt"
                && NAMED_SPAN_FILE_PREFIXES
                    .iter()
                    .any(|prefix| file_name.starts_with(prefix));
            if !is_named_span_file {
                continue;
            }
            if let Some(bytes) = self.0.read_attachment_file(path, MAX_NAMED_SPAN_FILE_BYTES)
                && let Ok(text) = std::str::from_utf8(&bytes)
            {
                // Captions and the transcript panel follow the words; the call window's
                // readings can freeze, so they are weighed apart (#131).
                if file_name.starts_with(CALL_WINDOW_FILE_PREFIX) {
                    spans.extend(parse_voice_cues(text));
                } else {
                    spans.extend(caption_spans(text));
                }
            }
        }
        spans
    }
}

/// The recordings whose speech a combined transcript was built from (#88), other than the meeting
/// holding it (#155).
///
/// The combined pass gives the meeting its own voices, so such a recording's live clusters, one
/// per chunk where the live matcher was unsure, are no longer anyone's to name.
/// Inputs: The repository. Outputs: The recordings; none when the repository cannot list them.
/// Side effects: Reads the meetings' combination index.
fn recordings_in_combined_transcripts<RepositoryPort>(
    note_repository: &RepositoryPort,
) -> std::collections::HashSet<MeetingId>
where
    RepositoryPort: NoteRepository + ?Sized,
{
    note_repository
        .meeting_recordings()
        .map(|recordings| {
            recordings
                .into_iter()
                .flat_map(|recording| {
                    let meeting_id = recording.meeting_id;
                    recording
                        .combined_with
                        .into_iter()
                        .filter(move |member| member != &meeting_id)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Loads the voice gallery: unresolved clusters of every meeting and every saved person.
///
/// Inputs: The persistence ports (either may be a `dyn` port).
/// Outputs and side effects: As `load_voice_library_with_names`, without name suggestions.
pub fn load_voice_library<RepositoryPort, VectorDatabase>(
    note_repository: &RepositoryPort,
    voice_vector_database: &VectorDatabase,
) -> Result<VoiceLibrary, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    load_voice_library_with_names(note_repository, voice_vector_database, &NoNamedSpans)
}

/// Loads the voice gallery, suggesting names from what call windows showed (#113).
///
/// Inputs: The persistence ports (either may be a `dyn` port) and the source of named spans.
/// Outputs: The library, or a storage error. No embedding or audio is included.
/// Side effects: Reads meetings, clusters, clips, identities, and variant counts. Nothing is
/// written to disk: a clip is only turned into a playable file when the owner asks to hear it.
/// For a meeting with named spans, its transcript segments are read to time each voice.
pub fn load_voice_library_with_names<RepositoryPort, VectorDatabase>(
    note_repository: &RepositoryPort,
    voice_vector_database: &VectorDatabase,
    named_spans: &dyn NamedSpanSource,
) -> Result<VoiceLibrary, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    let mut listed_clusters: Vec<VoiceLibraryCluster> = Vec::new();
    let mut representatives: Vec<Vec<f32>> = Vec::new();
    let superseded = recordings_in_combined_transcripts(note_repository);
    for meeting_id in note_repository
        .list_meeting_ids()
        .map_err(ClusteringError::from)?
    {
        if superseded.contains(&meeting_id) {
            continue;
        }
        let clusters = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default())
            .get_clusters_for_meeting(&meeting_id)?;
        if clusters.is_empty() {
            continue;
        }
        let meeting_title = note_repository
            .load_meeting_metadata(&meeting_id)
            .map_err(ClusteringError::from)?
            .and_then(|metadata| metadata.title().map(|title| title.as_str().to_owned()));
        let review_clips = note_repository
            .get_unassigned_clips(&meeting_id)
            .map_err(ClusteringError::from)?;
        let suggestions = suggested_names(note_repository, named_spans, &meeting_id)?;
        let labels = meeting_speaker_labels(note_repository, voice_vector_database, &meeting_id)?;
        for cluster in clusters {
            let has_review_clip = review_clips
                .iter()
                .any(|clip| clip.cluster_id() == Some(cluster.cluster_id()));
            representatives.push(cluster.representative_embedding().to_vec());
            listed_clusters.push(VoiceLibraryCluster {
                summary: UnknownClusterSummary::from_cluster(&cluster, &labels),
                meeting_title: meeting_title.clone(),
                has_review_clip,
                voice_group_id: cluster.cluster_id(),
                suggested_name: suggestions.get(&cluster.cluster_id()).cloned(),
            });
        }
    }
    let group_labels = group_by_suggested_name(
        &listed_clusters,
        group_unknown_voices(&representatives, DEFAULT_VOICE_GROUP_SIMILARITY),
    );
    drop(representatives);
    let mut unknown_voice_groups: Vec<UnknownVoiceGroup> = Vec::new();
    for (mut cluster, group_label) in listed_clusters.into_iter().zip(group_labels) {
        match unknown_voice_groups.get_mut(group_label) {
            Some(group) => {
                cluster.voice_group_id = group.group_id;
                group.clusters.push(cluster);
            }
            None => unknown_voice_groups.push(UnknownVoiceGroup {
                group_id: cluster.voice_group_id,
                clusters: vec![cluster],
            }),
        }
    }
    let unknown_clusters = unknown_voice_groups
        .iter()
        .flat_map(|group| group.clusters.iter().cloned())
        .collect();
    let mut saved_people = voice_vector_database
        .list_speaker_identities()
        .map_err(ClusteringError::from)?
        .iter()
        .map(|identity| {
            Ok(SavedPersonSummary {
                identity_id: identity.identity_id(),
                display_name: identity.display_name().to_owned(),
                voice_variant_count: voice_vector_database
                    .voice_variant_count(identity.identity_id())
                    .map_err(ClusteringError::from)?,
            })
        })
        .collect::<Result<Vec<_>, ClusterReviewError>>()?;
    saved_people.sort_by_key(|person| person.display_name.to_lowercase());
    Ok(VoiceLibrary {
        unknown_clusters,
        unknown_voice_groups,
        saved_people,
    })
}

/// Puts voices with the same suggested name in one group (#131).
///
/// A meeting can split one person into many short voices that don't sound alike enough to group;
/// the names shown while they spoke join them, so the owner names them once. A suggested name
/// outweighs sound: such a voice leaves its sound group. Names are compared ignoring case and
/// spacing.
///
/// Inputs: The voices and their sound-group numbers, in the same order.
/// Outputs: Group numbers from zero, numbered in order of first appearance. Side effects: None.
fn group_by_suggested_name(
    clusters: &[VoiceLibraryCluster],
    sound_groups: Vec<usize>,
) -> Vec<usize> {
    let mut keys: Vec<String> = Vec::new();
    let mut labels = Vec::with_capacity(sound_groups.len());
    for (cluster, sound_group) in clusters.iter().zip(sound_groups) {
        let key = match cluster.suggested_name.as_deref() {
            Some(name) => format!(
                "name:{}",
                name.split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
            ),
            None => format!("sound:{sound_group}"),
        };
        let label = match keys.iter().position(|known| *known == key) {
            Some(label) => label,
            None => {
                keys.push(key);
                keys.len() - 1
            }
        };
        labels.push(label);
    }
    labels
}

/// Suggests names for one meeting's unknown voices from its named spans (#113).
///
/// Outputs: Cluster ID to name; empty when the meeting has no named spans.
/// Side effects: Reads the named spans and, when there are any, the meeting's segments.
fn suggested_names<RepositoryPort>(
    note_repository: &RepositoryPort,
    named_spans: &dyn NamedSpanSource,
    meeting_id: &MeetingId,
) -> Result<std::collections::HashMap<Uuid, String>, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
{
    let spans = named_spans.named_spans(meeting_id);
    if spans.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let turns: Vec<VoiceTurn> = note_repository
        .load_transcript_segments(meeting_id)
        .map_err(ClusteringError::from)?
        .iter()
        .filter_map(|segment| {
            Some(VoiceTurn {
                cluster_id: segment.unknown_cluster_id()?,
                start_ms: segment.start_time_ms(),
                end_ms: segment.end_time_ms(),
            })
        })
        .collect();
    Ok(suggest_names(&turns, &spans))
}

/// Groups voices that sound like one person (#82).
///
/// Inputs: One representative voice per cluster, in any scale, and the least average cosine
/// similarity at which two groups merge (average linkage). A vector that is empty, has a zero or
/// non-finite length, or whose length differs from the others' is never grouped.
/// Outputs: One group number per voice, from zero, numbered in order of first appearance.
/// Side effects: None; quadratic in the number of voices.
pub fn group_unknown_voices(representatives: &[Vec<f32>], similarity_threshold: f32) -> Vec<usize> {
    let is_usable = |vector: &Vec<f32>| {
        let length_squared: f32 = vector.iter().map(|value| value * value).sum();
        !vector.is_empty() && length_squared.is_finite() && length_squared > 0.0
    };
    let mut labels: Vec<Option<usize>> = vec![None; representatives.len()];
    let mut next_label = 0_usize;
    let mut dimensions: Vec<usize> = Vec::new();
    for vector in representatives.iter().filter(|vector| is_usable(vector)) {
        if !dimensions.contains(&vector.len()) {
            dimensions.push(vector.len());
        }
    }
    for dimension in dimensions {
        let members: Vec<usize> = (0..representatives.len())
            .filter(|index| {
                representatives[*index].len() == dimension && is_usable(&representatives[*index])
            })
            .collect();
        let unit_vectors: Vec<Vec<f32>> = members
            .iter()
            .map(|index| unit_vector(&representatives[*index]))
            .collect();
        let member_labels = dense_labels(&average_linkage(&unit_vectors, similarity_threshold));
        let group_count = member_labels.iter().max().map_or(0, |highest| highest + 1);
        for (index, label) in members.into_iter().zip(member_labels) {
            labels[index] = Some(next_label + label);
        }
        next_label += group_count;
    }
    let labels: Vec<usize> = labels
        .into_iter()
        .map(|label| {
            label.unwrap_or_else(|| {
                next_label += 1;
                next_label - 1
            })
        })
        .collect();
    dense_labels(&labels)
}

/// Finds the review clip of one cluster, for playback on request.
///
/// Inputs: `cluster_id` names any cluster; an unknown one simply has no clip.
/// Outputs: The clip, `None` when there is none, or a storage error.
/// Side effects: Reads and decrypts the meeting's clips.
pub fn find_cluster_review_clip<RepositoryPort>(
    note_repository: &RepositoryPort,
    cluster_id: Uuid,
) -> Result<Option<UnassignedVoiceClip>, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
{
    let Some(cluster) = note_repository
        .load_unknown_speaker_cluster(cluster_id)
        .map_err(ClusteringError::from)?
    else {
        return Ok(None);
    };
    Ok(note_repository
        .get_unassigned_clips(cluster.meeting_id())
        .map_err(ClusteringError::from)?
        .into_iter()
        .find(|clip| clip.cluster_id() == Some(cluster_id)))
}

/// Assigns every chunk of a cluster to the identity named `identity_name`.
///
/// Inputs: `note_repository` and `voice_vector_database` are the persistence ports (either may be
/// a `dyn` port); `cluster_id` must name an unresolved cluster; `identity_name` must contain a
/// non-whitespace character.
/// Outputs: The assignment summary, or `BlankIdentityName`, or the clusterer's not-found,
/// already-resolved, or storage error.
/// Side effects: Reads identities, then (through the clusterer) saves the identity, stores one
/// voice variant per member embedding, relinks the meeting's transcript segments, resolves the
/// cluster, and deletes its review clips. Nothing is written when the name is blank.
pub fn assign_cluster_to_named_identity<RepositoryPort, VectorDatabase>(
    note_repository: &RepositoryPort,
    voice_vector_database: &VectorDatabase,
    cluster_id: Uuid,
    identity_name: &str,
) -> Result<NamedClusterAssignment, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    let trimmed_name = identity_name.trim();
    if trimmed_name.is_empty() {
        return Err(ClusterReviewError::BlankIdentityName);
    }
    let known_identity = voice_vector_database
        .list_speaker_identities()
        .map_err(ClusteringError::from)?
        .into_iter()
        .find(|identity| {
            identity.display_name().trim().to_lowercase() == trimmed_name.to_lowercase()
        });
    let identity =
        known_identity.unwrap_or_else(|| SpeakerIdentity::new(Uuid::new_v4(), trimmed_name));
    let assignment = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default())
        .assign_identity_to_cluster(&voice_vector_database, cluster_id, &identity)?;
    Ok(NamedClusterAssignment { assignment })
}

/// Names several clusters as one person, as when the owner names a voice group (#82).
///
/// Inputs: The persistence ports (either may be a `dyn` port), the clusters in the order to assign
/// them, and the person's name, which must contain a non-whitespace character.
/// Outputs: The summary. A cluster already named (for example on a paired device) or no longer
/// present is skipped. Errors: `BlankIdentityName` or `NoClustersSelected`, which write nothing;
/// the first skip's error when every cluster was skipped; and any storage error, which stops at
/// that cluster and leaves the earlier ones named.
/// Side effects: As `assign_cluster_to_named_identity`, once per cluster: saves the identity,
/// stores voice variants, relinks segments, resolves each cluster, and deletes its review clips.
pub fn assign_clusters_to_named_identity<RepositoryPort, VectorDatabase>(
    note_repository: &RepositoryPort,
    voice_vector_database: &VectorDatabase,
    cluster_ids: &[Uuid],
    identity_name: &str,
) -> Result<NamedGroupAssignment, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    if identity_name.trim().is_empty() {
        return Err(ClusterReviewError::BlankIdentityName);
    }
    if cluster_ids.is_empty() {
        return Err(ClusterReviewError::NoClustersSelected);
    }
    let mut outcome: Option<NamedGroupAssignment> = None;
    let mut first_skip: Option<ClusterReviewError> = None;
    let mut skipped_cluster_count = 0_usize;
    for cluster_id in cluster_ids {
        match assign_cluster_to_named_identity(
            note_repository,
            voice_vector_database,
            *cluster_id,
            identity_name,
        ) {
            Ok(named) => {
                let summary = outcome.get_or_insert_with(|| NamedGroupAssignment {
                    identity: named.assignment.identity().clone(),
                    identity_created: named.identity_created(),
                    assigned_cluster_ids: Vec::new(),
                    skipped_cluster_count: 0,
                    variants_stored: 0,
                    segments_updated: 0,
                });
                summary.assigned_cluster_ids.push(named.cluster_id());
                summary.variants_stored += named.variants_stored();
                summary.segments_updated += named.segments_updated();
            }
            Err(
                error @ ClusterReviewError::Clustering(
                    ClusteringError::ClusterNotFound { .. }
                    | ClusteringError::ClusterAlreadyResolved { .. },
                ),
            ) => {
                skipped_cluster_count += 1;
                first_skip.get_or_insert(error);
            }
            Err(error) => return Err(error),
        }
    }
    match (outcome, first_skip) {
        (Some(mut summary), _) => {
            summary.skipped_cluster_count = skipped_cluster_count;
            Ok(summary)
        }
        (None, Some(error)) => Err(error),
        (None, None) => Err(ClusterReviewError::NoClustersSelected),
    }
}
