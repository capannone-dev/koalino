//! Specifies when the after-meeting pass runs and what happens to the retained audio (#87).

use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    adapters::audio::{
        chunk_audio_store::ChunkAudioStore, retained_audio_store::RetainedAudioStore,
    },
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        transcript_refinement::ChunkSpan,
    },
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, MeetingId, TimedWord, Transcript, TranscriptSegment,
            TranscriptionContext,
        },
    },
    ports::{ai::TranscriptionEngine, crypto::EmbeddingCipher, storage::NoteRepository},
    refinement_worker::{RefinementControl, RefinementWorker},
    storage_composition::open_note_repository,
};

/// Answers every window with "nuovo testo", or fails every call.
struct Engine {
    fails: bool,
    calls: Arc<AtomicUsize>,
}

impl TranscriptionEngine for Engine {
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fails {
            return Err(TranscriptionError::Unavailable);
        }
        Ok(
            Transcript::new(audio_recording.meeting_id().clone(), "nuovo testo").with_timed_words(
                vec![
                    TimedWord::new("nuovo", 0, 300),
                    TimedWord::new("testo", 300, 600),
                ],
            ),
        )
    }
}

/// A database with one meeting and its live line, and a store holding its speech.
struct Setup {
    folder: PathBuf,
    database: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
    store: Arc<RetainedAudioStore>,
    meeting: MeetingId,
}

impl Setup {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!("nt-pass-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
        let database = folder.join("notes.sqlite3");
        let cipher: Arc<dyn EmbeddingCipher> =
            Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([5; 32])));
        let store = Arc::new(RetainedAudioStore::new(
            folder.join("retained-audio"),
            Arc::clone(&cipher),
        ));
        let meeting = MeetingId::new("meeting-pass");
        let setup = Self {
            folder,
            database,
            cipher,
            store,
            meeting,
        };
        let repository = setup.repository();
        repository
            .create_meeting(&setup.meeting, &[])
            .unwrap_or_else(|error| panic!("{error}"));
        repository
            .save_transcript_segments(
                &setup.meeting,
                &[TranscriptSegment::new(
                    setup.meeting.clone(),
                    0,
                    900,
                    None,
                    "vecchio",
                )],
            )
            .unwrap_or_else(|error| panic!("{error}"));
        setup
            .store
            .append_chunk(
                &setup.meeting,
                ChunkSpan {
                    start_ms: 0,
                    end_ms: 1_000,
                },
                None,
                &[1; 16_000],
            )
            .unwrap_or_else(|error| panic!("{error}"));
        setup
    }

    fn repository(&self) -> crate::adapters::storage::sqlite_adapter::SqliteNoteRepository {
        open_note_repository(
            &self.database,
            DeviceId::new("device-a"),
            Arc::clone(&self.cipher),
        )
        .unwrap_or_else(|error| panic!("{error:#}"))
    }

    fn worker(
        &self,
        control: &Arc<RefinementControl>,
        fails: bool,
        calls: &Arc<AtomicUsize>,
    ) -> RefinementWorker {
        let calls = Arc::clone(calls);
        let database = self.database.clone();
        let cipher = Arc::clone(&self.cipher);
        RefinementWorker::new(
            Arc::clone(&self.store),
            Arc::clone(control),
            Box::new(move |_audio: Arc<ChunkAudioStore>| {
                Ok(Box::new(Engine {
                    fails,
                    calls: Arc::clone(&calls),
                }) as Box<dyn TranscriptionEngine>)
            }),
            Box::new(move || {
                Ok(Box::new(open_note_repository(
                    &database,
                    DeviceId::new("device-a"),
                    Arc::clone(&cipher),
                )?) as Box<dyn NoteRepository>)
            }),
            DeviceId::new("device-a"),
        )
    }

    fn texts(&self) -> Vec<String> {
        self.repository()
            .load_transcript_segments(&self.meeting)
            .unwrap_or_else(|error| panic!("{error}"))
            .iter()
            .map(|segment| segment.text().to_owned())
            .collect()
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.folder);
    }
}

#[test]
fn a_pass_replaces_the_text_and_deletes_the_audio() {
    let setup = Setup::new();
    let control = Arc::new(RefinementControl::new(false));
    let calls = Arc::new(AtomicUsize::new(0));

    let refined = setup.worker(&control, false, &calls).run_pending();

    assert_eq!(refined, 1);
    assert_eq!(setup.texts(), vec!["nuovo testo"]);
    assert!(setup.store.pending_meetings().is_empty());
}

#[test]
fn nothing_runs_while_recording() {
    let setup = Setup::new();
    let control = Arc::new(RefinementControl::new(false));
    control.set_recording(true);
    let calls = Arc::new(AtomicUsize::new(0));

    assert_eq!(setup.worker(&control, false, &calls).run_pending(), 0);

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(setup.texts(), vec!["vecchio"]);
    assert_eq!(setup.store.pending_meetings(), vec![setup.meeting.clone()]);
}

#[test]
fn a_phone_waits_for_charging() {
    let setup = Setup::new();
    let control = Arc::new(RefinementControl::new(true));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut worker = setup.worker(&control, false, &calls);

    assert_eq!(worker.run_pending(), 0, "not charging");
    control.set_charging(true);
    assert_eq!(worker.run_pending(), 1, "charging");
    assert_eq!(setup.texts(), vec!["nuovo testo"]);
}

#[test]
fn three_failures_delete_the_audio_and_keep_the_live_text() {
    let setup = Setup::new();
    let control = Arc::new(RefinementControl::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut worker = setup.worker(&control, true, &calls);

    for attempt in 1..=2 {
        worker.run_pending();
        assert_eq!(
            setup.store.pending_meetings().len(),
            1,
            "kept after attempt {attempt}"
        );
    }
    worker.run_pending();

    assert!(setup.store.pending_meetings().is_empty());
    assert_eq!(setup.texts(), vec!["vecchio"]);
}

#[test]
fn a_deleted_meeting_loses_its_audio_without_a_pass() {
    let setup = Setup::new();
    setup
        .repository()
        .delete_meeting(&setup.meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    let control = Arc::new(RefinementControl::new(false));
    let calls = Arc::new(AtomicUsize::new(0));

    setup.worker(&control, false, &calls).run_pending();

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(setup.store.pending_meetings().is_empty());
}

#[test]
fn deleting_a_meeting_removes_its_retained_audio() {
    use crate::{application::deletion::MeetingDeletionHooks, cli_recording::LocalMeetingFiles};
    let setup = Setup::new();
    assert_eq!(setup.store.pending_meetings(), vec![setup.meeting.clone()]);

    LocalMeetingFiles::for_database(&setup.database).remove_files(&setup.meeting);

    assert!(setup.store.pending_meetings().is_empty());
}
