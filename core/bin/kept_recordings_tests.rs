//! Specifies the owner's kept meeting recordings (#140): the switch, the playback WAV with each
//! chunk at its time, folded recordings shifted onto the meeting, the commands, and wiping.

use std::{fs, path::PathBuf, sync::Arc};

use serde_json::{Value, json};

use crate::{
    adapters::audio::retained_audio_store::RetainedAudioStore,
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        transcript_refinement::ChunkSpan,
    },
    domain::models::MeetingId,
    kept_recordings::{
        RecordingSource, RecordingsServices, keeps_recordings, kept_recordings_directory,
        run_recordings_command, write_wav,
    },
    ports::crypto::EmbeddingCipher,
    transcription_settings::settings_file_for,
};

/// A fresh folder holding `db.sqlite3`, with services over it and its kept store.
struct Setup {
    folder: PathBuf,
    database: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
}

impl Setup {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!("nt-kept-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
        Self {
            database: folder.join("db.sqlite3"),
            folder,
            cipher: Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
        }
    }

    fn services(&self) -> RecordingsServices {
        RecordingsServices::for_database(
            &self.database,
            Arc::clone(&self.cipher),
            self.folder.join("playback"),
        )
    }

    fn kept(&self) -> RetainedAudioStore {
        RetainedAudioStore::new(
            kept_recordings_directory(&self.database),
            Arc::clone(&self.cipher),
        )
    }

    /// Keeps a chunk of `value` samples from `start_ms` for `meeting`.
    fn keep(&self, meeting: &MeetingId, start_ms: u64, length_ms: u64, value: i16) {
        self.kept()
            .append_chunk(
                meeting,
                ChunkSpan {
                    start_ms,
                    end_ms: start_ms + length_ms,
                },
                None,
                &vec![value; (length_ms * 16) as usize],
            )
            .unwrap_or_else(|error| panic!("{error}"));
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.folder);
    }
}

/// The samples of a WAV written by `write_wav`.
fn samples_of(wav: &[u8]) -> Vec<i16> {
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[36..40], b"data");
    let length = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]) as usize;
    assert_eq!(
        wav.len(),
        44 + length,
        "the header states the data's length"
    );
    wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect()
}

#[test]
fn chunks_play_at_their_time_with_silence_between() {
    let first = [100_i16; 16];
    let second = [200_i16; 32];
    let mut wav = Vec::new();
    let duration = write_wav(&mut [(3, &second[..]), (0, &first[..])], &mut wav)
        .unwrap_or_else(|e| panic!("{e}"));

    let samples = samples_of(&wav);
    assert_eq!(duration, 5);
    assert_eq!(samples.len(), 80);
    assert!(samples[..16].iter().all(|sample| *sample == 100));
    assert!(
        samples[16..48].iter().all(|sample| *sample == 0),
        "a 2 ms gap"
    );
    assert!(samples[48..].iter().all(|sample| *sample == 200));
}

#[test]
fn overlapping_chunks_keep_the_earlier_ones_samples() {
    let first = [1_i16; 32];
    let second = [2_i16; 32];
    let mut wav = Vec::new();
    write_wav(&mut [(0, &first[..]), (1, &second[..])], &mut wav).unwrap_or_else(|e| panic!("{e}"));

    let samples = samples_of(&wav);
    assert_eq!(samples.len(), 48);
    assert!(samples[..32].iter().all(|sample| *sample == 1));
    assert!(samples[32..].iter().all(|sample| *sample == 2));
}

#[test]
fn the_switch_is_off_until_turned_on_and_is_saved_per_device() {
    let setup = Setup::new();
    let services = setup.services();
    assert_eq!(services.status_json()["keep"], false);

    let answer = run_recordings_command(&services, None, r#"{"command":"set_keep","keep":true}"#);

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["keep"], true);
    assert!(keeps_recordings(&settings_file_for(&setup.database)));
}

#[test]
fn playback_writes_one_private_wav_and_release_removes_it() {
    let setup = Setup::new();
    let meeting = MeetingId::new("m-1");
    setup.keep(&meeting, 1_000, 500, 1_000);
    let services = setup.services();

    let has = run_recordings_command(&services, None, r#"{"command":"has","meeting_id":"m-1"}"#);
    assert_eq!(has["available"], true);
    let answer = run_recordings_command(
        &services,
        None,
        r#"{"command":"prepare","meeting_id":"m-1"}"#,
    );

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["duration_ms"], 1_500);
    let path = PathBuf::from(answer["path"].as_str().unwrap_or_default());
    let samples = samples_of(&fs::read(&path).unwrap_or_else(|error| panic!("{error}")));
    assert_eq!(samples.len(), 24_000);
    assert!(
        samples[..16_000].iter().all(|sample| *sample == 0),
        "silence until 1 s"
    );
    assert!(
        samples[16_000..].iter().any(|sample| *sample != 0),
        "then the speech"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).map(|metadata| metadata.permissions().mode() & 0o777);
        assert_eq!(mode.ok(), Some(0o600));
    }

    run_recordings_command(&services, None, r#"{"command":"release"}"#);
    assert!(!path.exists());
}

#[test]
fn a_meeting_without_a_kept_recording_says_so() {
    let setup = Setup::new();
    let answer = run_recordings_command(
        &setup.services(),
        None,
        r#"{"command":"prepare","meeting_id":"none"}"#,
    );
    assert_eq!(answer["error_code"], "not_found");
}

#[test]
fn a_folded_recording_plays_shifted_onto_the_meeting() {
    let setup = Setup::new();
    let partner = MeetingId::new("phone-meeting");
    setup.keep(&partner, 0, 100, 500);
    let services = setup.services();
    let mut wav = Vec::new();

    let duration = services
        .write_recording(
            &[RecordingSource {
                meeting_id: partner,
                offset_ms: 2_000,
            }],
            &mut wav,
        )
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(duration, 2_100);
}

#[test]
fn wiping_deletes_every_kept_recording_and_reports_the_space() {
    let setup = Setup::new();
    setup.keep(&MeetingId::new("a"), 0, 1_000, 300);
    setup.keep(&MeetingId::new("b"), 0, 1_000, 300);
    let services = setup.services();
    let status = services.status_json();
    assert_eq!(status["count"], 2);
    let bytes = status["bytes"].as_u64().unwrap_or_default();
    assert!(bytes > 0);

    let answer: Value = run_recordings_command(&services, None, r#"{"command":"wipe"}"#);

    assert_eq!(answer, json!({"ok": true, "deleted": 2, "bytes": bytes}));
    assert_eq!(services.status_json()["count"], 0);
}
