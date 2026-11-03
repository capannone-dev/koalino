//! Specifies the clean-up of voices that a recording's final pass left behind (#82).
//!
//! Tests use in-memory SQLite and short hand-written vectors; no model is loaded.

use chrono::Utc;
use uuid::Uuid;

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        deletion::{MeetingDeletionHooks, remove_leftover_voices},
        unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
    },
    domain::models::{
        MeetingId, TranscriptSegment, UnknownSpeakerChunk, UnknownSpeakerCluster,
        meeting_voice_chunk_id,
    },
    ports::storage::NoteRepository,
};

/// Reports one meeting as being recorded.
struct RecordingOne(MeetingId);

impl MeetingDeletionHooks for RecordingOne {
    fn is_recording(&self, meeting_id: &MeetingId) -> bool {
        *meeting_id == self.0
    }

    fn remove_files(&self, _meeting_id: &MeetingId) {}
}

/// Opens an empty in-memory repository.
fn repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"))
}

/// Adds a live cluster of one chunk to `meeting_id`. Outputs: its ID.
fn live_cluster(repository: &SqliteNoteRepository, meeting_id: &MeetingId, axis: usize) -> Uuid {
    let mut embedding = vec![0.0; 4];
    embedding[axis] = 1.0;
    UnknownSpeakerClusterer::new(repository, ClusteringConfig::default())
        .find_or_create_cluster(meeting_id, &embedding, Uuid::new_v4().to_string(), 900)
        .unwrap_or_else(|error| panic!("{error}"))
        .cluster_id()
}

/// Adds a final-pass cluster holding one whole-meeting voice. Outputs: its ID.
fn final_cluster(repository: &SqliteNoteRepository, meeting_id: &MeetingId) -> Uuid {
    let cluster_id = Uuid::new_v4();
    let chunk_id = meeting_voice_chunk_id(cluster_id);
    repository
        .replace_unknown_speaker_cluster_members(
            &UnknownSpeakerCluster::new(
                cluster_id,
                meeting_id.clone(),
                vec![0.0, 0.0, 0.0, 1.0],
                vec![chunk_id.clone()],
                30_000,
                Utc::now(),
                None,
            ),
            &[UnknownSpeakerChunk::new(
                chunk_id,
                vec![0.0, 0.0, 0.0, 1.0],
                30_000,
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    cluster_id
}

/// Creates a meeting whose transcript points at `referenced` clusters.
fn meeting(repository: &SqliteNoteRepository, name: &str) -> MeetingId {
    let meeting_id = MeetingId::new(name);
    repository
        .create_meeting(&meeting_id, &[])
        .unwrap_or_else(|error| panic!("{error}"));
    meeting_id
}

/// Points one transcript line at each of `cluster_ids`.
fn reference(repository: &SqliteNoteRepository, meeting_id: &MeetingId, cluster_ids: &[Uuid]) {
    let segments: Vec<TranscriptSegment> = cluster_ids
        .iter()
        .enumerate()
        .map(|(index, cluster_id)| {
            let start_ms = index as u64 * 1_000;
            TranscriptSegment::new(meeting_id.clone(), start_ms, start_ms + 900, None, "ciao")
                .with_unknown_cluster_id(Some(*cluster_id))
        })
        .collect();
    repository
        .save_transcript_segments(meeting_id, &segments)
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Lists a meeting's unresolved cluster IDs.
fn listed(repository: &SqliteNoteRepository, meeting_id: &MeetingId) -> Vec<Uuid> {
    repository
        .load_unknown_speaker_clusters(meeting_id)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(UnknownSpeakerCluster::cluster_id)
        .collect()
}

/// Verifies that, once a recording's final pass has run, voices no transcript line belongs to go
/// for good, also on paired devices, while the final voices and referenced live voices stay.
#[test]
fn leftovers_of_a_finished_recording_are_deleted_for_good() {
    let repository = repository();
    let finished = meeting(&repository, "meeting-finished");
    let kept_final = final_cluster(&repository, &finished);
    let kept_live = live_cluster(&repository, &finished, 0);
    let leftover = live_cluster(&repository, &finished, 1);
    reference(&repository, &finished, &[kept_final, kept_live]);

    let removed =
        remove_leftover_voices(&repository, None).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(removed, 1);
    assert_eq!(listed(&repository, &finished), vec![kept_final, kept_live]);
    let records = repository
        .deletion_records()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        records.cluster_ids,
        vec![leftover],
        "paired devices delete it too"
    );
}

/// Verifies nothing is deleted where the final pass may not have run, or a recording is running.
#[test]
fn leftovers_stay_without_a_final_pass_or_during_a_recording() {
    let repository = repository();
    let crashed = meeting(&repository, "meeting-crashed");
    let live = live_cluster(&repository, &crashed, 0);
    let wordless = live_cluster(&repository, &crashed, 1);
    reference(&repository, &crashed, &[live]);
    let recording = meeting(&repository, "meeting-recording");
    let final_voice = final_cluster(&repository, &recording);
    let fresh = live_cluster(&repository, &recording, 2);
    reference(&repository, &recording, &[final_voice]);

    let removed = remove_leftover_voices(&repository, Some(&RecordingOne(recording.clone())))
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(removed, 0);
    assert_eq!(listed(&repository, &crashed), vec![live, wordless]);
    assert_eq!(listed(&repository, &recording), vec![final_voice, fresh]);
}
