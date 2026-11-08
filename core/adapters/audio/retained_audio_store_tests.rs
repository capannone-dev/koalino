//! Specifies the sealed on-disk copy of a meeting's speech kept for the after-meeting pass (#87).

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::retained_audio_store::{RetainedAudioError, RetainedAudioStore};
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        transcript_refinement::ChunkSpan,
    },
    domain::{models::MeetingId, transcription_language::TranscriptionLanguage},
};

/// A fresh, empty folder for one test.
fn folder() -> PathBuf {
    let path = std::env::temp_dir().join(format!("nt-retained-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap_or_else(|error| panic!("{error}"));
    path
}

fn store(path: &Path) -> RetainedAudioStore {
    RetainedAudioStore::new(
        path.to_path_buf(),
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([7; 32]))),
    )
}

/// Recognisable samples, so a plaintext copy on disk would be found.
fn samples(seed: i16, count: usize) -> Vec<i16> {
    (0..count)
        .map(|index| seed.wrapping_add((index as i16).wrapping_mul(37)))
        .collect()
}

#[test]
fn chunks_come_back_in_order_with_their_place_in_the_meeting() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-1");
    let first = samples(11, 1_600);
    let second = samples(-5, 3_200);

    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 0,
                end_ms: 100,
            },
            None,
            &first,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 500,
                end_ms: 700,
            },
            Some(TranscriptionLanguage::Code("it")),
            &second,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let chunks = store
        .load(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(chunks.len(), 2);
    assert_eq!(
        chunks[0].span(),
        ChunkSpan {
            start_ms: 0,
            end_ms: 100
        }
    );
    assert_eq!(chunks[0].samples(), first.as_slice());
    assert_eq!(
        chunks[1].span(),
        ChunkSpan {
            start_ms: 500,
            end_ms: 700
        }
    );
    assert_eq!(chunks[1].samples(), second.as_slice());
    assert_eq!(chunks[0].language(), None);
    assert_eq!(
        chunks[1].language(),
        Some(TranscriptionLanguage::Code("it"))
    );
    let _ = fs::remove_dir_all(path);
}

#[test]
fn the_file_holds_no_plain_audio() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-plain");
    let audio = samples(1_234, 4_000);
    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 0,
                end_ms: 250,
            },
            None,
            &audio,
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let plain: Vec<u8> = audio[100..140]
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let on_disk: Vec<u8> = fs::read_dir(&path)
        .unwrap_or_else(|error| panic!("{error}"))
        .flat_map(|entry| fs::read(entry.unwrap_or_else(|error| panic!("{error}")).path()))
        .flatten()
        .collect();

    assert!(!on_disk.is_empty());
    assert!(
        !on_disk
            .windows(plain.len())
            .any(|window| window == plain.as_slice())
    );
    let _ = fs::remove_dir_all(path);
}

#[test]
fn deleting_removes_the_meeting_and_lists_only_the_others() {
    let path = folder();
    let store = store(&path);
    let kept = MeetingId::new("meeting-kept");
    let deleted = MeetingId::new("meeting/with odd:chars");
    for meeting in [&kept, &deleted] {
        store
            .append_chunk(
                meeting,
                ChunkSpan {
                    start_ms: 0,
                    end_ms: 10,
                },
                None,
                &samples(1, 160),
            )
            .unwrap_or_else(|error| panic!("{error}"));
    }

    assert!(
        store
            .delete(&deleted)
            .unwrap_or_else(|error| panic!("{error}"))
    );
    assert!(
        !store
            .delete(&deleted)
            .unwrap_or_else(|error| panic!("{error}"))
    );

    assert_eq!(store.pending_meetings(), vec![kept.clone()]);
    assert!(
        store
            .load(&deleted)
            .unwrap_or_else(|error| panic!("{error}"))
            .is_empty()
    );
    let _ = fs::remove_dir_all(path);
}

#[test]
fn audio_moved_to_another_meeting_is_refused() {
    let path = folder();
    let store = store(&path);
    let source = MeetingId::new("meeting-source");
    let target = MeetingId::new("meeting-target");
    store
        .append_chunk(
            &source,
            ChunkSpan {
                start_ms: 0,
                end_ms: 10,
            },
            None,
            &samples(3, 160),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    store
        .append_chunk(
            &target,
            ChunkSpan {
                start_ms: 0,
                end_ms: 10,
            },
            None,
            &samples(4, 160),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let file_of = |meeting: &MeetingId| path.join(RetainedAudioStore::file_name(meeting));
    fs::copy(file_of(&source), file_of(&target)).unwrap_or_else(|error| panic!("{error}"));

    assert!(matches!(
        store.load(&target),
        Err(RetainedAudioError::Unreadable)
    ));
    let _ = fs::remove_dir_all(path);
}

#[test]
fn a_truncated_file_is_refused() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-cut");
    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 0,
                end_ms: 10,
            },
            None,
            &samples(3, 160),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let file = path.join(RetainedAudioStore::file_name(&meeting));
    let bytes = fs::read(&file).unwrap_or_else(|error| panic!("{error}"));
    fs::write(&file, &bytes[..bytes.len() - 3]).unwrap_or_else(|error| panic!("{error}"));

    assert!(matches!(
        store.load(&meeting),
        Err(RetainedAudioError::Unreadable)
    ));
    let _ = fs::remove_dir_all(path);
}

#[test]
fn received_speech_is_hidden_until_complete_then_replaces_any_earlier_copy() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-phone");
    let span = ChunkSpan {
        start_ms: 0,
        end_ms: 1_000,
    };

    store
        .begin_incoming(&meeting)
        .and_then(|()| store.append_incoming(&meeting, span, None, &samples(3, 160)))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(store.pending_meetings().is_empty(), "not complete yet");

    store
        .finish_incoming(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(store.pending_meetings(), vec![meeting.clone()]);
    assert!(store.written_at(&meeting).is_some());

    // A second copy starts over and replaces the first when complete.
    store
        .begin_incoming(&meeting)
        .and_then(|()| store.append_incoming(&meeting, span, None, &samples(9, 80)))
        .and_then(|()| store.finish_incoming(&meeting))
        .unwrap_or_else(|error| panic!("{error}"));
    let chunks = store
        .load(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].samples(), samples(9, 80).as_slice());
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn deleting_also_drops_an_unfinished_copy() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-phone");
    store
        .begin_incoming(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));

    store
        .delete(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));

    let left = fs::read_dir(&path).map(Iterator::count).unwrap_or(1);
    assert_eq!(left, 0);
    let _ = fs::remove_dir_all(&path);
}

#[cfg(unix)]
#[test]
fn retained_files_are_readable_by_their_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-a");
    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 0,
                end_ms: 10,
            },
            None,
            &samples(1, 16),
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let mode = fs::metadata(path.join(RetainedAudioStore::file_name(&meeting)))
        .map(|metadata| metadata.permissions().mode() & 0o777)
        .unwrap_or_default();
    assert_eq!(mode, 0o600);
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn a_meeting_remembers_its_pass_alone_until_new_speech_or_deletion() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-1");
    let span = ChunkSpan {
        start_ms: 0,
        end_ms: 100,
    };
    store
        .append_chunk(&meeting, span, None, &samples(1, 160))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!store.passed_alone(&meeting));

    store
        .mark_passed_alone(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(store.passed_alone(&meeting));
    assert_eq!(store.pending_meetings(), vec![meeting.clone()]);

    // The recording went on: the pass must run again.
    store
        .append_chunk(&meeting, span, None, &samples(2, 160))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!store.passed_alone(&meeting));

    store
        .mark_passed_alone(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    store
        .delete(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!store.passed_alone(&meeting));
    assert_eq!(
        fs::read_dir(&path)
            .map(|entries| entries.count())
            .unwrap_or_default(),
        0
    );
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn the_speech_index_reports_held_speech_without_the_key() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-1");
    let index = super::retained_audio_store::RetainedSpeechIndex::new(path.clone());
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3_600);
    assert_eq!(index.held_speech(&meeting, later), None);

    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 0,
                end_ms: 100,
            },
            None,
            &samples(1, 160),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    store
        .mark_passed_alone(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));

    let held = index
        .held_speech(&meeting, later)
        .unwrap_or_else(|| panic!("the speech must be reported"));
    assert!(held.passed_alone);
    assert!(held.age >= std::time::Duration::from_secs(3_590));
    let _ = fs::remove_dir_all(&path);
}
