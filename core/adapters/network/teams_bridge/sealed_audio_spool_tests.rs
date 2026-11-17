//! Specifies the sealed spool that holds Teams audio the pump hasn't reached yet (#133).

use std::path::PathBuf;

use crate::{
    adapters::network::teams_bridge::{
        bridge_buffer::AudioSpool, sealed_audio_spool::SealedAudioSpool,
    },
    application::crypto::voice_encryptor::ephemeral_cipher,
};

/// Creates an empty folder for one test.
fn new_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "note-taker-audio-spool-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// Lists the files in a folder.
fn files_in(folder: &PathBuf) -> Vec<PathBuf> {
    std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn audio_comes_back_whole_and_leaves_no_file() {
    let folder = new_folder("roundtrip");
    let spool = SealedAudioSpool::open(&folder, ephemeral_cipher(), 1 << 20)
        .unwrap_or_else(|error| panic!("spool should open: {error}"));
    let audio: Vec<u8> = (0..32_000_u32).map(|index| (index % 251) as u8).collect();

    let ticket = spool
        .store(&audio)
        .unwrap_or_else(|| panic!("audio should be kept"));
    let stored = files_in(&folder);
    assert_eq!(stored.len(), 1);
    let on_disk = std::fs::read(&stored[0]).unwrap_or_default();
    assert!(
        !on_disk.windows(64).any(|window| window == &audio[..64]),
        "the audio is sealed on disk"
    );

    assert_eq!(spool.take(&ticket), Some(audio));
    assert!(files_in(&folder).is_empty());
    assert_eq!(spool.take(&ticket), None);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn audio_beyond_the_cap_is_refused() {
    let folder = new_folder("cap");
    let spool = SealedAudioSpool::open(&folder, ephemeral_cipher(), 1_000)
        .unwrap_or_else(|error| panic!("spool should open: {error}"));
    let first = spool.store(&[1; 600]).unwrap_or_else(|| panic!("fits"));
    assert!(spool.store(&[2; 600]).is_none());
    assert!(spool.take(&first).is_some());
    assert!(spool.store(&[2; 600]).is_some(), "taking frees room");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn audio_left_by_an_earlier_run_is_deleted() {
    // Raw audio must never outlive its processing: a crash or quit leaves spooled chunks behind.
    let folder = new_folder("leftover");
    let first = SealedAudioSpool::open(&folder, ephemeral_cipher(), 1 << 20)
        .unwrap_or_else(|error| panic!("spool should open: {error}"));
    first.store(&[7; 100]).unwrap_or_else(|| panic!("kept"));
    drop(first);
    assert_eq!(files_in(&folder).len(), 1);

    let _second = SealedAudioSpool::open(&folder, ephemeral_cipher(), 1 << 20)
        .unwrap_or_else(|error| panic!("spool should open: {error}"));
    assert!(files_in(&folder).is_empty());
    let _ = std::fs::remove_dir_all(&folder);
}
