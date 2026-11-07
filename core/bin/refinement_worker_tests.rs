//! Specifies when the after-meeting pass runs and what happens to the retained audio (#87), and how
//! devices fold, send and combine their recordings of one meeting (#88).

use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime},
};

use zeroize::Zeroizing;

use crate::{
    adapters::audio::{
        chunk_audio_store::ChunkAudioStore, retained_audio_store::RetainedAudioStore,
    },
    application::{
        combined_meetings::{COMBINING_GRACE, KEEP_FOR_COMBINING},
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
    ports::{
        ai::TranscriptionEngine,
        crypto::EmbeddingCipher,
        meeting_document::MeetingCombination,
        speech_transfer::{
            SpeechChunk, SpeechInbox, SpeechPart, SpeechSender, SpeechTransferError,
        },
        storage::NoteRepository,
    },
    refinement_worker::{RefinementControl, RefinementWorker, RetainedSpeechInbox},
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
        .with_clock(after(COMBINING_GRACE))
    }

    /// Opens the database as `device`.
    fn repository_of(
        &self,
        device: &str,
    ) -> crate::adapters::storage::sqlite_adapter::SqliteNoteRepository {
        open_note_repository(
            &self.database,
            DeviceId::new(device),
            Arc::clone(&self.cipher),
        )
        .unwrap_or_else(|error| panic!("{error:#}"))
    }

    /// Records `meeting` on `device` with the shared talk as its live transcript.
    fn record_talk(&self, meeting: &MeetingId, device: &str, first_ms: u64) {
        let repository = self.repository_of(device);
        if meeting != &self.meeting {
            repository
                .create_meeting(meeting, &[])
                .unwrap_or_else(|error| panic!("{error}"));
        }
        let segments: Vec<TranscriptSegment> = TALK
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let start = first_ms + index as u64 * 4_000;
                TranscriptSegment::new(meeting.clone(), start, start + 3_800, None, *line)
            })
            .collect();
        repository
            .save_transcript_segments(meeting, &segments)
            .unwrap_or_else(|error| panic!("{error}"));
    }

    /// Retains one second of speech for `meeting`.
    fn retain(&self, meeting: &MeetingId) {
        self.store
            .append_chunk(
                meeting,
                ChunkSpan {
                    start_ms: 0,
                    end_ms: 1_000,
                },
                None,
                &[1; 16_000],
            )
            .unwrap_or_else(|error| panic!("{error}"));
    }

    /// Opens the repository for the inbox and the phone worker.
    fn opener(&self, device: &str) -> crate::refinement_worker::RepositoryFactory {
        let database = self.database.clone();
        let cipher = Arc::clone(&self.cipher);
        let device = DeviceId::new(device);
        Box::new(move || {
            Ok(Box::new(open_note_repository(
                &database,
                device.clone(),
                Arc::clone(&cipher),
            )?) as Box<dyn NoteRepository>)
        })
    }

    fn combination_of(&self, meeting: &MeetingId) -> Option<MeetingCombination> {
        self.repository()
            .meeting_recordings()
            .unwrap_or_else(|error| panic!("{error}"))
            .into_iter()
            .find(|recording| recording.meeting_id == *meeting)
            .and_then(|recording| recording.combined_into)
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

/// A clock `ahead` of the real one.
fn after(ahead: Duration) -> Box<dyn Fn() -> SystemTime + Send> {
    Box::new(move || SystemTime::now() + ahead)
}

/// Lines of one conversation, one per 4 s.
const TALK: [&str; 8] = [
    "Buongiorno a tutti, iniziamo con il bilancio del trimestre.",
    "Le vendite sono cresciute del dieci per cento rispetto a marzo.",
    "Però i costi di spedizione sono aumentati più del previsto.",
    "Dobbiamo rinegoziare il contratto con il corriere entro giugno.",
    "Chi se ne occupa? Io posso chiamarli già domani mattina.",
    "Perfetto, poi passiamo al lancio del nuovo prodotto.",
    "Il prototipo è pronto ma mancano ancora i test sul campo.",
    "Possiamo coinvolgere tre clienti di Milano per la prova.",
];

/// Records the parts it is given, or answers every part with `fails`.
struct Sender {
    parts: Mutex<Vec<(DeviceId, MeetingId, u32, u32)>>,
    fails: Option<SpeechTransferError>,
}

impl SpeechSender for Sender {
    fn send_part(&self, to: &DeviceId, part: SpeechPart) -> Result<(), SpeechTransferError> {
        if let Some(error) = self.fails {
            return Err(error);
        }
        self.parts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                to.clone(),
                part.meeting_id,
                part.part_index,
                part.part_count,
            ));
        Ok(())
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

#[test]
fn a_meeting_recorded_on_two_devices_is_folded_and_combined_once() {
    let setup = Setup::new();
    let phone_meeting = MeetingId::new("meeting-phone");
    setup.record_talk(&setup.meeting, "device-a", 3_000);
    setup.record_talk(&phone_meeting, "device-b", 1_000);
    // The phone's speech has arrived.
    setup.retain(&phone_meeting);
    let control = Arc::new(RefinementControl::new(false));
    let calls = Arc::new(AtomicUsize::new(0));

    let refined = setup.worker(&control, false, &calls).run_pending();

    assert_eq!(refined, 1, "one pass for both recordings");
    let combination = setup
        .combination_of(&phone_meeting)
        .unwrap_or_else(|| panic!("the phone's recording must be folded"));
    assert_eq!(combination.into, setup.meeting);
    assert_eq!(combination.combine_on, DeviceId::new("device-a"));
    assert_eq!(calls.load(Ordering::SeqCst), 2, "a window from each device");
    assert!(setup.texts().iter().any(|text| text.contains("nuovo")));
    assert!(setup.store.pending_meetings().is_empty());
}

#[test]
fn the_combining_device_waits_for_the_other_devices_speech() {
    let setup = Setup::new();
    let phone_meeting = MeetingId::new("meeting-phone");
    setup.record_talk(&setup.meeting, "device-a", 3_000);
    setup.record_talk(&phone_meeting, "device-b", 1_000);
    let control = Arc::new(RefinementControl::new(false));
    let calls = Arc::new(AtomicUsize::new(0));

    assert_eq!(setup.worker(&control, false, &calls).run_pending(), 0);

    assert!(setup.combination_of(&phone_meeting).is_some());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(setup.store.pending_meetings(), vec![setup.meeting.clone()]);
}

/// A phone ("device-a") whose meeting was folded into the laptop's ("device-b").
fn phone_setup() -> (Setup, Arc<RefinementControl>) {
    let setup = Setup::new();
    let laptop_meeting = MeetingId::new("meeting-laptop");
    setup.record_talk(&laptop_meeting, "device-b", 3_000);
    setup.record_talk(&setup.meeting, "device-a", 1_000);
    (setup, Arc::new(RefinementControl::new(true)))
}

#[test]
fn a_phone_folds_its_recording_and_sends_its_speech_without_charging() {
    let (setup, control) = phone_setup();
    let sender = Arc::new(Sender {
        parts: Mutex::new(Vec::new()),
        fails: None,
    });
    control.set_speech_sender(Arc::clone(&sender) as Arc<dyn SpeechSender>);
    let mut worker = RefinementWorker::without_pass(
        Arc::clone(&setup.store),
        Arc::clone(&control),
        setup.opener("device-a"),
        DeviceId::new("device-a"),
    );

    assert_eq!(worker.run_pending(), 0);

    let combination = setup
        .combination_of(&setup.meeting)
        .unwrap_or_else(|| panic!("the phone's recording must be folded"));
    assert_eq!(combination.combine_on, DeviceId::new("device-b"));
    let parts = sender
        .parts
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        parts,
        vec![(DeviceId::new("device-b"), setup.meeting.clone(), 0, 1)]
    );
    assert!(setup.store.pending_meetings().is_empty());
}

#[test]
fn speech_that_was_not_delivered_is_kept_until_a_day_has_passed() {
    let (setup, control) = phone_setup();
    control.set_speech_sender(Arc::new(Sender {
        parts: Mutex::new(Vec::new()),
        fails: Some(SpeechTransferError::Unreachable),
    }));
    let worker = |ahead: Duration| {
        RefinementWorker::without_pass(
            Arc::clone(&setup.store),
            Arc::clone(&control),
            setup.opener("device-a"),
            DeviceId::new("device-a"),
        )
        .with_clock(after(ahead))
    };

    worker(Duration::ZERO).run_pending();
    assert_eq!(setup.store.pending_meetings(), vec![setup.meeting.clone()]);

    worker(KEEP_FOR_COMBINING).run_pending();
    assert!(setup.store.pending_meetings().is_empty());
}

fn speech_part(meeting: &MeetingId, part_index: u32, part_count: u32) -> SpeechPart {
    SpeechPart {
        meeting_id: meeting.clone(),
        part_index,
        part_count,
        chunks: vec![SpeechChunk {
            start_ms: u64::from(part_index) * 1_000,
            end_ms: u64::from(part_index) * 1_000 + 1_000,
            language: None,
            samples: Zeroizing::new(vec![2; 16_000]),
        }],
    }
}

#[test]
fn the_inbox_takes_only_speech_of_meetings_combined_here_in_order() {
    let setup = Setup::new();
    let phone_meeting = MeetingId::new("meeting-phone");
    setup.record_talk(&setup.meeting, "device-a", 3_000);
    setup.record_talk(&phone_meeting, "device-b", 1_000);
    let control = Arc::new(RefinementControl::new(false));
    let mut inbox = RetainedSpeechInbox::new(
        Arc::clone(&setup.store),
        setup.opener("device-a"),
        DeviceId::new("device-a"),
        Arc::clone(&control),
    );
    let phone = DeviceId::new("device-b");

    assert_eq!(
        inbox.receive_part(&phone, speech_part(&phone_meeting, 0, 2)),
        Err(SpeechTransferError::Refused),
        "not folded yet"
    );
    setup
        .repository()
        .combine_meeting(
            &phone_meeting,
            &MeetingCombination {
                into: setup.meeting.clone(),
                offset_ms: 2_000,
                combine_on: DeviceId::new("device-a"),
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        inbox.receive_part(&phone, speech_part(&phone_meeting, 1, 2)),
        Err(SpeechTransferError::Refused),
        "out of order"
    );
    assert_eq!(
        inbox.receive_part(
            &DeviceId::new("device-c"),
            speech_part(&phone_meeting, 0, 2)
        ),
        Err(SpeechTransferError::Refused),
        "from a device that did not record it"
    );
    assert_eq!(
        inbox.receive_part(&phone, speech_part(&phone_meeting, 0, 2)),
        Ok(())
    );
    assert_eq!(
        inbox.receive_part(&phone, speech_part(&phone_meeting, 0, 2)),
        Ok(())
    );
    assert!(
        !setup.store.pending_meetings().contains(&phone_meeting),
        "not available before the last part"
    );
    assert_eq!(
        inbox.receive_part(&phone, speech_part(&phone_meeting, 1, 2)),
        Ok(())
    );

    assert!(setup.store.pending_meetings().contains(&phone_meeting));
    let chunks = setup
        .store
        .load(&phone_meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(chunks.len(), 2);
}
