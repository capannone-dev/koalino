//! Specifies the sealed on-disk copy of a meeting's speech kept for the after-meeting pass (#87).

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use super::{
    opus_speech::{decode_speech, encode_speech},
    opus_speech_tests::correlation,
    retained_audio_store::{RetainedAudioError, RetainedAudioStore},
};
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        transcript_refinement::ChunkSpan,
    },
    domain::{models::MeetingId, transcription_language::TranscriptionLanguage},
    ports::crypto::EmbeddingCipher,
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

/// Recognisable, voice-like samples: a voice whose pitch depends on `seed`, at 16 kHz.
fn samples(seed: i16, count: usize) -> Vec<i16> {
    let pitch = 120.0 + f32::from(seed.unsigned_abs() % 64) * 5.0;
    (0..count)
        .map(|index| {
            let time = index as f32 / 16_000.0;
            let voiced: f32 = (1..=8)
                .map(|harmonic| {
                    (2.0 * std::f32::consts::PI * pitch * harmonic as f32 * time).sin()
                        / harmonic as f32
                })
                .sum();
            (voiced * 6_000.0) as i16
        })
        .collect()
}

/// `samples(seed, count)` compressed as a paired device sends it.
fn encoded(seed: i16, count: usize) -> Vec<u8> {
    encode_speech(&samples(seed, count))
        .unwrap_or_else(|error| panic!("{error}"))
        .to_vec()
}

/// Asserts that speech read back is the speech written: as long, and (when long enough to judge)
/// the same sound, since compression keeps the sound rather than every sample.
fn assert_sounds_like(read_back: &[i16], written: &[i16]) {
    assert_eq!(read_back.len(), written.len());
    if written.len() >= 1_600 {
        let similarity = correlation(written, read_back, 0);
        assert!(similarity > 0.8, "correlation {similarity}");
    }
}

/// Writes a record as builds before Opus did: raw samples under the v1 context.
fn append_raw_record(path: &Path, meeting: &MeetingId, span: ChunkSpan, samples: &[i16]) {
    let mut plaintext = Vec::new();
    plaintext.extend_from_slice(&span.start_ms.to_le_bytes());
    plaintext.extend_from_slice(&span.end_ms.to_le_bytes());
    plaintext.push(0);
    for sample in samples {
        plaintext.extend_from_slice(&sample.to_le_bytes());
    }
    let sealed = VoiceEncryptor::new(&EncryptionKey::from_bytes([7; 32]))
        .seal_secret(
            &format!("retained-audio:v1:{}", meeting.as_str()),
            &plaintext,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let mut record = u32::try_from(sealed.len())
        .unwrap_or_else(|error| panic!("{error}"))
        .to_le_bytes()
        .to_vec();
    record.extend_from_slice(&sealed);
    let file = path.join(RetainedAudioStore::file_name(meeting));
    let mut bytes = fs::read(&file).unwrap_or_default();
    bytes.extend_from_slice(&record);
    fs::write(&file, bytes).unwrap_or_else(|error| panic!("{error}"));
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
    assert_sounds_like(chunks[0].samples(), &first);
    assert_eq!(
        chunks[1].span(),
        ChunkSpan {
            start_ms: 500,
            end_ms: 700
        }
    );
    assert_sounds_like(chunks[1].samples(), &second);
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
        .and_then(|()| store.append_incoming_encoded(&meeting, span, None, &encoded(3, 160)))
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
        .and_then(|()| store.append_incoming_encoded(&meeting, span, None, &encoded(9, 80)))
        .and_then(|()| store.finish_incoming(&meeting))
        .unwrap_or_else(|error| panic!("{error}"));
    let chunks = store
        .load(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(chunks.len(), 1);
    assert_sounds_like(chunks[0].samples(), &samples(9, 80));
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

#[test]
fn speech_is_kept_compressed_to_a_fraction_of_its_raw_size() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-long");
    let audio = samples(2, 160_000);

    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 0,
                end_ms: 10_000,
            },
            None,
            &audio,
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let on_disk = fs::metadata(path.join(RetainedAudioStore::file_name(&meeting)))
        .map(|metadata| metadata.len())
        .unwrap_or_else(|error| panic!("{error}"));
    let raw_bytes = (audio.len() * 2) as u64;
    assert!(
        on_disk * 8 <= raw_bytes,
        "{on_disk} bytes for {raw_bytes} raw"
    );
    let _ = fs::remove_dir_all(path);
}

#[test]
fn raw_speech_kept_by_an_earlier_version_still_loads_next_to_compressed_speech() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-upgraded");
    let raw = samples(4, 3_200);
    let later = samples(8, 3_200);
    append_raw_record(
        &path,
        &meeting,
        ChunkSpan {
            start_ms: 0,
            end_ms: 200,
        },
        &raw,
    );

    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 200,
                end_ms: 400,
            },
            None,
            &later,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let chunks = store
        .load(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].samples(), raw.as_slice(), "raw speech is exact");
    assert_sounds_like(chunks[1].samples(), &later);
    let _ = fs::remove_dir_all(path);
}

#[test]
fn speech_for_sending_comes_compressed_once_whatever_version_kept_it() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-send");
    let raw = samples(5, 3_200);
    let compressed = samples(6, 4_800);
    append_raw_record(
        &path,
        &meeting,
        ChunkSpan {
            start_ms: 0,
            end_ms: 200,
        },
        &raw,
    );
    store
        .append_chunk(
            &meeting,
            ChunkSpan {
                start_ms: 200,
                end_ms: 500,
            },
            Some(TranscriptionLanguage::Code("it")),
            &compressed,
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let chunks = store
        .load_encoded(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(chunks.len(), 2);
    assert_eq!(
        chunks[1].span(),
        ChunkSpan {
            start_ms: 200,
            end_ms: 500
        }
    );
    assert_eq!(
        chunks[1].language(),
        Some(TranscriptionLanguage::Code("it"))
    );
    let first = decode_speech(chunks[0].speech()).unwrap_or_else(|error| panic!("{error}"));
    let second = decode_speech(chunks[1].speech()).unwrap_or_else(|error| panic!("{error}"));
    assert_sounds_like(&first, &raw);
    assert_sounds_like(&second, &compressed);
    // The kept Opus is sent as it is, not decoded and compressed again.
    let loaded = store
        .load(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(second.as_slice(), loaded[1].samples());
    let _ = fs::remove_dir_all(path);
}

#[test]
fn received_speech_that_is_not_valid_opus_is_refused() {
    let path = folder();
    let store = store(&path);
    let meeting = MeetingId::new("meeting-damaged");
    let span = ChunkSpan {
        start_ms: 0,
        end_ms: 100,
    };
    let mut damaged = encoded(1, 1_600);
    damaged.truncate(damaged.len() - 3);

    store
        .begin_incoming(&meeting)
        .unwrap_or_else(|error| panic!("{error}"));
    let result = store.append_incoming_encoded(&meeting, span, None, &damaged);

    assert!(matches!(result, Err(RetainedAudioError::Unreadable)));
    let _ = fs::remove_dir_all(path);
}
