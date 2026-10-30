//! Specifies the P2P JSON the Flutter Devices screen reads and sends through the FFI (Issue #43).
//!
//! The request checks run without a task; the pairing test runs two real P2P tasks that listen on
//! loopback and find each other through configured peer addresses, like the daemon VM tests.

use std::{collections::HashMap, sync::Arc, time::Duration};

use serde_json::Value;
use uuid::Uuid;

use super::{p2p_command_json, p2p_status_json};
use crate::{
    adapters::storage::{
        attachment_file_store::AttachmentFileStore, sqlite_voice_vector_db::SqliteVoiceVectorDB,
    },
    application::{
        compute_router::ComputePreference,
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_crdt::MeetingCrdtService,
        p2p_service::P2pControl,
        transcription_routing::RemoteTranscriptionGateway,
    },
    domain::{
        errors::TranscriptionError,
        models::{DeviceId, MeetingId, SpeakerIdentity, VoiceEmbedding},
    },
    p2p_composition::{P2pConfig, P2pHandle, spawn_p2p_task},
    ports::crypto::EmbeddingCipher,
    ports::{storage::AttachmentRepository, voice::VoiceVectorDB},
    storage_composition::open_note_repository,
};

/// Parses a JSON answer.
fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("answer must be JSON: {error}"))
}

/// Without a P2P task the status says so and nothing else.
#[test]
fn the_status_without_a_task_is_disabled() {
    assert_eq!(
        json(&p2p_status_json(None)),
        serde_json::json!({"enabled": false})
    );
}

/// Malformed requests are refused before they reach a task; a valid one without a task is
/// reported as unavailable.
#[test]
fn malformed_requests_are_refused_and_a_missing_task_is_unavailable() {
    for (request, error_code) in [
        ("not json", "invalid_request"),
        (r#"{"command":"format_disk"}"#, "invalid_request"),
        (r#"{"command":"start_pairing"}"#, "invalid_request"),
        (
            r#"{"command":"make_pairable","extra":1}"#,
            "invalid_request",
        ),
        (
            r#"{"command":"start_pairing","peer_id":"12D3 KooW"}"#,
            "invalid_peer_id",
        ),
        (r#"{"command":"revoke","peer_id":""}"#, "invalid_peer_id"),
        (r#"{"command":"make_pairable"}"#, "unavailable"),
        (
            r#"{"command":"confirm_code","peer_id":"12D3KooWA"}"#,
            "unavailable",
        ),
    ] {
        let answer = json(&p2p_command_json(None, request));
        assert_eq!(answer["ok"], false, "{request}");
        assert_eq!(answer["error_code"], error_code, "{request}");
    }
}

/// Starts a P2P task on loopback over a fresh database, with `static_peers` configured.
fn start_task(name: &str, static_peers: &[String]) -> P2pHandle {
    start_task_at(&fresh_database(name), name, static_peers)
}

/// Picks a database path in a new scratch directory.
fn fresh_database(name: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!("ffi-p2p-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    directory.join("notes.sqlite3")
}

/// The cipher every test database is sealed with.
fn test_cipher() -> Arc<dyn EmbeddingCipher> {
    Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([9; 32])))
}

/// Starts a P2P task on loopback over the database at `database_path`.
fn start_task_at(
    database_path: &std::path::Path,
    name: &str,
    static_peers: &[String],
) -> P2pHandle {
    let variables: HashMap<&str, String> = HashMap::from([
        ("NOTE_TAKER_P2P_LISTEN", "/ip4/127.0.0.1/tcp/0".to_owned()),
        ("NOTE_TAKER_DEVICE_NAME", name.to_owned()),
        ("NOTE_TAKER_P2P_PEERS", static_peers.join(",")),
    ]);
    let config = P2pConfig::from_lookup(|variable| variables.get(variable).cloned())
        .unwrap_or_else(|error| panic!("config should parse: {error:#}"))
        .unwrap_or_else(|| panic!("P2P should be enabled"));
    spawn_p2p_task(
        database_path,
        DeviceId::new(format!("device-{name}")),
        test_cipher(),
        config,
        None,
    )
    .unwrap_or_else(|error| panic!("task must start: {error:#}"))
}

/// Polls `handle`'s status JSON until `condition` holds, for up to ten seconds.
fn wait_for_status(handle: &P2pHandle, condition: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..200 {
        let status = json(&p2p_status_json(Some(handle as &dyn P2pControl)));
        if condition(&status) {
            return status;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    json(&p2p_status_json(Some(handle as &dyn P2pControl)))
}

/// Runs one command through the JSON interface.
fn command(handle: &P2pHandle, request: &str) -> Value {
    json(&p2p_command_json(Some(handle as &dyn P2pControl), request))
}

/// Two devices pair through the JSON commands: one opens pairing, the other starts it, both show
/// the same code, and once both confirm each lists the other as paired.
#[test]
fn two_devices_pair_through_the_json_commands() {
    let laptop = start_task("laptop", &[]);
    let laptop_status = wait_for_status(&laptop, |status| {
        status["listen_addresses"]
            .as_array()
            .is_some_and(|addresses| !addresses.is_empty())
    });
    let laptop_id = laptop_status["local_peer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let laptop_address = format!(
        "{}/p2p/{laptop_id}",
        laptop_status["listen_addresses"][0]
            .as_str()
            .unwrap_or_default()
    );
    let phone = start_task("phone", &[laptop_address]);
    let phone_id = json(&p2p_status_json(Some(&phone as &dyn P2pControl)))["local_peer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    let opened = command(&laptop, r#"{"command":"make_pairable"}"#);
    let nearby = wait_for_status(&phone, |status| {
        status["nearby_devices"].as_array().is_some_and(|devices| {
            devices
                .iter()
                .any(|device| device["peer_id"] == laptop_id.as_str())
        })
    });
    let started = command(
        &phone,
        &format!(r#"{{"command":"start_pairing","peer_id":"{laptop_id}"}}"#),
    );
    let requested = wait_for_status(&laptop, |status| {
        status["incoming_requests"]
            .as_array()
            .is_some_and(|requests| !requests.is_empty())
    });
    let confirmed = command(
        &phone,
        &format!(r#"{{"command":"confirm_code","peer_id":"{laptop_id}"}}"#),
    );
    let accepted = command(
        &laptop,
        &format!(r#"{{"command":"accept_request","peer_id":"{phone_id}"}}"#),
    );
    let phone_paired = wait_for_status(&phone, |status| {
        status["trusted_devices"]
            .as_array()
            .is_some_and(|devices| !devices.is_empty())
    });
    let revoked = command(
        &laptop,
        &format!(r#"{{"command":"revoke","peer_id":"{phone_id}"}}"#),
    );
    laptop.shutdown();
    phone.shutdown();

    assert_eq!(opened, serde_json::json!({"ok": true}));
    assert!(
        nearby["nearby_devices"]
            .as_array()
            .is_some_and(|devices| !devices.is_empty()),
        "the configured laptop must be offered for pairing: {nearby}"
    );
    assert_eq!(started["ok"], true, "{started}");
    let code = started["code"].as_str().unwrap_or_default();
    assert_eq!(code.len(), 6, "{started}");
    assert_eq!(
        requested["incoming_requests"][0]["code"], code,
        "{requested}"
    );
    assert_eq!(requested["incoming_requests"][0]["device_name"], "phone");
    assert_eq!(confirmed["ok"], true, "{confirmed}");
    assert_eq!(accepted["ok"], true, "{accepted}");
    assert_eq!(accepted["update"]["kind"], "paired", "{accepted}");
    assert_eq!(
        phone_paired["trusted_devices"][0]["peer_id"],
        laptop_id.as_str()
    );
    assert_eq!(phone_paired["trusted_devices"][0]["device_name"], "laptop");
    assert_eq!(revoked, serde_json::json!({"ok": true, "revoked": true}));
}

/// Starts two tasks over the given databases and pairs them through the JSON commands.
///
/// Outputs: The laptop's and the phone's handles, with pairing confirmed on both sides.
/// Side effects: Binds loopback sockets and writes to both databases.
fn pair_tasks(
    laptop_database: &std::path::Path,
    phone_database: &std::path::Path,
) -> (P2pHandle, P2pHandle) {
    let laptop = start_task_at(laptop_database, "laptop", &[]);
    let laptop_status = wait_for_status(&laptop, |status| {
        status["listen_addresses"]
            .as_array()
            .is_some_and(|addresses| !addresses.is_empty())
    });
    let laptop_id = laptop_status["local_peer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let laptop_address = format!(
        "{}/p2p/{laptop_id}",
        laptop_status["listen_addresses"][0]
            .as_str()
            .unwrap_or_default()
    );
    let phone = start_task_at(phone_database, "phone", &[laptop_address]);
    let phone_id = json(&p2p_status_json(Some(&phone as &dyn P2pControl)))["local_peer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    command(&laptop, r#"{"command":"make_pairable"}"#);
    wait_for_status(&phone, |status| {
        status["nearby_devices"].as_array().is_some_and(|devices| {
            devices
                .iter()
                .any(|device| device["peer_id"] == laptop_id.as_str())
        })
    });
    command(
        &phone,
        &format!(r#"{{"command":"start_pairing","peer_id":"{laptop_id}"}}"#),
    );
    wait_for_status(&laptop, |status| {
        status["incoming_requests"]
            .as_array()
            .is_some_and(|requests| !requests.is_empty())
    });
    command(
        &phone,
        &format!(r#"{{"command":"confirm_code","peer_id":"{laptop_id}"}}"#),
    );
    command(
        &laptop,
        &format!(r#"{{"command":"accept_request","peer_id":"{phone_id}"}}"#),
    );
    (laptop, phone)
}

/// The composed task keeps the voice library in step (Issue #56): a speaker enrolled before pairing
/// reaches the other device over real TCP, sealed in its database.
#[test]
fn the_composed_task_syncs_the_voice_library() {
    let laptop_database = fresh_database("laptop");
    let phone_database = fresh_database("phone");
    let alice = Uuid::from_u128(0xA1);
    let on_laptop = SqliteVoiceVectorDB::open(&laptop_database, test_cipher())
        .unwrap_or_else(|error| panic!("voice store must open: {error}"));
    on_laptop
        .save_speaker_identity(&SpeakerIdentity::new(alice, "Alice"))
        .and_then(|()| {
            on_laptop.store_embedding(VoiceEmbedding::new(
                alice,
                Uuid::from_u128(0xA2),
                vec![0.6, 0.8],
            ))
        })
        .unwrap_or_else(|error| panic!("enrollment must succeed: {error}"));
    let (laptop, phone) = pair_tasks(&laptop_database, &phone_database);

    let on_phone = SqliteVoiceVectorDB::open(&phone_database, test_cipher())
        .unwrap_or_else(|error| panic!("voice store must open: {error}"));
    let mut variants = 0;
    for _ in 0..300 {
        variants = on_phone.voice_variant_count(alice).unwrap_or_default();
        if variants > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    laptop.shutdown();
    phone.shutdown();

    assert_eq!(variants, 1);
    let found = on_phone
        .search_similar(&[0.6, 0.8], 1)
        .unwrap_or_else(|error| panic!("the phone must open what it stored: {error}"));
    assert_eq!(found[0].0.identity_id(), alice);
}

/// A photo attached on the laptop reaches the phone through the composed tasks over TCP, stored in
/// the folder beside the phone's database.
#[test]
fn the_composed_task_syncs_attachments() {
    let laptop_database = fresh_database("laptop");
    let phone_database = fresh_database("phone");
    let laptop_folder = laptop_database
        .parent()
        .unwrap_or_else(|| panic!("the database has a folder"));
    let meeting_id = MeetingId::new("meeting-photo");
    let on_laptop = open_note_repository(
        &laptop_database,
        DeviceId::new("device-laptop"),
        test_cipher(),
    )
    .unwrap_or_else(|error| panic!("laptop repository must open: {error:#}"));
    MeetingCrdtService::new(&on_laptop)
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
    let photo: Vec<u8> = (0..150_000_u32)
        .map(|value| value.to_le_bytes()[1])
        .collect();
    let source = laptop_folder.join("photo.jpg");
    std::fs::write(&source, &photo)
        .unwrap_or_else(|error| panic!("source must be written: {error}"));
    AttachmentFileStore::new(
        laptop_folder.join("attachments"),
        DeviceId::new("device-laptop"),
    )
    .import_file(&on_laptop, &meeting_id, &source, Some(7_000))
    .unwrap_or_else(|error| panic!("the photo must attach: {error}"));

    let (laptop, phone) = pair_tasks(&laptop_database, &phone_database);
    let on_phone = open_note_repository(
        &phone_database,
        DeviceId::new("device-phone"),
        test_cipher(),
    )
    .unwrap_or_else(|error| panic!("phone repository must open: {error:#}"));
    let mut received = Vec::new();
    for _ in 0..300 {
        received = on_phone
            .get_attachments_for_meeting(&meeting_id)
            .unwrap_or_default();
        if !received.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    laptop.shutdown();
    phone.shutdown();

    assert_eq!(received.len(), 1);
    assert_eq!(received[0].meeting_time_ms(), Some(7_000));
    assert!(
        std::path::Path::new(received[0].file_path()).starts_with(
            phone_database
                .parent()
                .unwrap_or_else(|| panic!("the database has a folder"))
                .join("attachments")
        )
    );
    assert_eq!(std::fs::read(received[0].file_path()).ok(), Some(photo));
}

/// The choice goes through the JSON commands and shows in the status; asking a paired device with
/// no transcription engine fails quickly, so the pipeline can fall back.
#[test]
fn the_composed_task_routes_transcription_and_reports_a_refusal() {
    let laptop_database = fresh_database("laptop");
    let phone_database = fresh_database("phone");
    let (laptop, phone) = pair_tasks(&laptop_database, &phone_database);
    let laptop_id = laptop.snapshot().status.local_peer_id;

    let status = wait_for_status(&phone, |status| {
        status["trusted_devices"]
            .as_array()
            .is_some_and(|devices| !devices.is_empty())
    });
    assert_eq!(status["transcription_route"], "local");
    let chosen = command(
        &phone,
        &format!(
            r#"{{"command":"set_transcription_route","route":"{}"}}"#,
            laptop_id.as_str()
        ),
    );
    assert_eq!(chosen["ok"], true, "{chosen}");
    assert_eq!(
        command(
            &phone,
            r#"{"command":"set_transcription_route","route":"../bad"}"#
        )["error_code"],
        "invalid_peer_id"
    );
    wait_for_status(&phone, |status| {
        status["transcription_route"] == laptop_id.as_str()
            && status["trusted_devices"][0]["capabilities"].is_object()
    });
    assert_eq!(
        RemoteTranscriptionGateway::route(&phone),
        ComputePreference::SpecificPeer(laptop_id.clone())
    );

    // The pair key is agreed moments after pairing; until then the answer is also a refusal.
    let started = std::time::Instant::now();
    let answer = phone.transcribe_on_peer(&laptop_id, vec![1, 2, 3], None, None);
    laptop.shutdown();
    phone.shutdown();

    assert!(matches!(answer, Err(TranscriptionError::Unavailable)));
    assert!(started.elapsed() < Duration::from_secs(30));
}
