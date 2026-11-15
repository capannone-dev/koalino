//! Specifies the P2P JSON the Flutter Devices screen reads and sends through the FFI (Issue #43).
//!
//! The request checks run without a task; the pairing test runs two real P2P tasks that listen on
//! loopback and find each other through configured peer addresses, like the daemon VM tests.

use std::{collections::HashMap, sync::Arc, time::Duration};

use serde_json::Value;
use uuid::Uuid;

use super::{p2p_command_json, p2p_command_json_at, p2p_status_json};
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
        models::{DeviceId, MeetingId, SpeakerIdentity, TranscriptSegment, VoiceEmbedding},
    },
    p2p_composition::{P2pConfig, P2pHandle, spawn_p2p_task},
    ports::crypto::EmbeddingCipher,
    ports::{
        speech_transfer::{
            SpeechChunk, SpeechInbox, SpeechLink, SpeechPart, SpeechSender, SpeechTransferError,
        },
        storage::{AttachmentRepository, NoteRepository},
        voice::VoiceVectorDB,
    },
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

/// Tells whether a status lists a device as nearby. Real discovery runs here, so other devices on
/// the machine's networks can be listed too.
fn lists_nearby(status: &Value, peer_id: &str) -> bool {
    status["nearby_devices"]
        .as_array()
        .is_some_and(|devices| devices.iter().any(|device| device["peer_id"] == peer_id))
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
        on_laptop.sealed_files(),
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
    // Sealed on the phone with its key (#99); the clear contents crossed the encrypted link.
    assert!(
        crate::adapters::storage::sealed_files::is_sealed(std::path::Path::new(
            received[0].file_path()
        ))
        .unwrap_or(false)
    );
    assert_eq!(
        on_phone
            .sealed_files()
            .read_all(std::path::Path::new(received[0].file_path()))
            .ok()
            .map(|contents| contents.to_vec()),
        Some(photo)
    );
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

/// Stores the meetings and part positions it receives.
struct RecordingInbox(Arc<std::sync::Mutex<Vec<(MeetingId, u32, usize)>>>);

impl SpeechInbox for RecordingInbox {
    fn receive_part(
        &mut self,
        _from: &DeviceId,
        part: SpeechPart,
    ) -> Result<(), SpeechTransferError> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((
                part.meeting_id,
                part.part_index,
                part.chunks[0].speech.len(),
            ));
        Ok(())
    }
}

fn speech_part() -> SpeechPart {
    SpeechPart {
        meeting_id: MeetingId::new("meeting-phone"),
        part_index: 0,
        part_count: 1,
        chunks: vec![SpeechChunk {
            start_ms: 0,
            end_ms: 1_000,
            language: None,
            speech: zeroize::Zeroizing::new(vec![3; 16_000]),
        }],
    }
}

/// The composed tasks carry a meeting's speech to the paired device that combines it (#88): it
/// is refused until that device's worker attaches its inbox, then stored and confirmed.
#[test]
fn the_composed_task_carries_speech_to_the_combining_device() {
    let laptop_database = fresh_database("laptop");
    let phone_database = fresh_database("phone");
    let (laptop, phone) = pair_tasks(&laptop_database, &phone_database);
    // Meetings name devices by their storage ID, which `start_task_at` derives from the name.
    let laptop_id = DeviceId::new("device-laptop");
    let received = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Until the pair key is agreed the part is unreachable; then refused without an inbox.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut before_inbox = phone.send_part(&laptop_id, speech_part());
    while before_inbox == Err(SpeechTransferError::Unreachable)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(200));
        before_inbox = phone.send_part(&laptop_id, speech_part());
    }
    laptop.accept_speech_with(Box::new(RecordingInbox(Arc::clone(&received))));
    let after_inbox = phone.send_part(&laptop_id, speech_part());
    laptop.shutdown();
    phone.shutdown();

    assert_eq!(before_inbox, Err(SpeechTransferError::Refused));
    assert_eq!(after_inbox, Ok(()));
    assert_eq!(
        *received
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![(MeetingId::new("meeting-phone"), 0, 16_000)]
    );
}

/// Verifies Settings' addresses are checked, apply live, and survive a restart.
#[test]
fn network_addresses_are_validated_and_survive_a_restart() {
    let database_path = fresh_database("addresses");
    let relay = "/ip4/192.0.2.1/tcp/4001/p2p/12D3KooWGzBXAxb7K8mqbh3XwXgXzmvGD2QK1ep2zV7EMTwQgbQa";
    let task = start_task_at(&database_path, "addresses", &[]);

    let bad_relay = command(
        &task,
        r#"{"command":"set_network_addresses","relay":"not an address","peers":[]}"#,
    );
    let circuit_relay = command(
        &task,
        &format!(
            r#"{{"command":"set_network_addresses","relay":"{relay}/p2p-circuit/p2p/{}","peers":[]}}"#,
            "12D3KooWGzBXAxb7K8mqbh3XwXgXzmvGD2QK1ep2zV7EMTwQgbQa"
        ),
    );
    let bad_peer = command(
        &task,
        r#"{"command":"set_network_addresses","peers":["/ip4/192.0.2.1/tcp/4001"]}"#,
    );
    let saved = command(
        &task,
        &format!(r#"{{"command":"set_network_addresses","relay":"{relay}","peers":[" "]}}"#),
    );
    let live = wait_for_status(&task, |status| {
        status["network"]["relay_state"] == "not_paired"
    });
    task.shutdown();
    let restarted = start_task_at(&database_path, "addresses", &[]);
    let after_restart = wait_for_status(&restarted, |status| {
        status["network"]["relay_address"] == relay
    });

    assert_eq!(bad_relay["error_code"], "invalid_relay_address");
    assert_eq!(circuit_relay["error_code"], "invalid_relay_address");
    assert_eq!(bad_peer["error_code"], "invalid_peer_address");
    assert_eq!(saved["ok"], true);
    assert_eq!(live["network"]["relay_address"], relay);
    assert_eq!(live["network"]["peers"], serde_json::json!([]));
    assert_eq!(after_restart["network"]["relay_state"], "not_paired");
}

/// Verifies two devices with no addresses for each other pair with only an invite and the code,
/// that a stranger's invite grants nothing without the code, and that the route survives a restart.
#[test]
fn an_invite_finds_a_device_but_pairing_still_needs_the_code() {
    let inviter = start_task("inviter", &[]);
    let joiner_database = fresh_database("joiner");
    let joiner = start_task_at(&joiner_database, "joiner", &[]);

    let own_invite = command(&inviter, r#"{"command":"create_invite"}"#);
    let invite = own_invite["invite"].as_str().unwrap_or_default().to_owned();
    let own_join = command(
        &inviter,
        &serde_json::json!({"command":"join_invite","invite":invite}).to_string(),
    );
    let malformed = command(&joiner, r#"{"command":"join_invite","invite":"hello"}"#);
    let expired = command(
        &joiner,
        &serde_json::json!({
            "command": "join_invite",
            "invite": "note-taker-invite:1;12D3KooWPeer;1;/ip4/192.0.2.1/tcp/4001",
        })
        .to_string(),
    );
    let unchanged = wait_for_status(&joiner, |_| true);
    let joined = command(
        &joiner,
        &serde_json::json!({"command":"join_invite","invite":invite}).to_string(),
    );
    let inviter_id = joined["peer_id"].as_str().unwrap_or_default().to_owned();
    let listed = wait_for_status(&joiner, |status| lists_nearby(status, &inviter_id));

    assert_eq!(own_invite["ok"], true);
    assert!(own_invite["expires_at_ms"].as_u64().is_some());
    assert_eq!(own_join["error_code"], "invite_from_this_device");
    assert_eq!(malformed["error_code"], "invite_malformed");
    assert_eq!(expired["error_code"], "invite_expired");
    assert!(!lists_nearby(&unchanged, "12D3KooWPeer"));
    assert_eq!(listed["trusted_devices"], serde_json::json!([]));

    let started = command(
        &joiner,
        &format!(r#"{{"command":"start_pairing","peer_id":"{inviter_id}"}}"#),
    );
    let requested = wait_for_status(&inviter, |status| {
        status["incoming_requests"][0]["code"] == started["code"]
    });
    let joiner_id = requested["incoming_requests"][0]["peer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(started["ok"], true);
    assert_eq!(requested["trusted_devices"], serde_json::json!([]));
    command(
        &inviter,
        &format!(r#"{{"command":"accept_request","peer_id":"{joiner_id}"}}"#),
    );
    // The owner takes a moment to confirm, so what the inviter sends once paired is dropped here.
    std::thread::sleep(Duration::from_secs(1));
    command(
        &joiner,
        &format!(r#"{{"command":"confirm_code","peer_id":"{inviter_id}"}}"#),
    );
    let paired = wait_for_status(&joiner, |status| {
        !status["trusted_devices"][0]["capabilities"].is_null()
    });
    assert_eq!(paired["trusted_devices"][0]["peer_id"], inviter_id.as_str());
    assert!(
        !paired["trusted_devices"][0]["capabilities"].is_null(),
        "a connected device must not read as not connected"
    );

    // The invited address must be stored by the next tick, so a restart can reconnect.
    std::thread::sleep(Duration::from_millis(1_500));
    joiner.shutdown();
    let restarted = start_task_at(&joiner_database, "joiner", &[]);
    let reconnected = wait_for_status(&restarted, |status| {
        !status["trusted_devices"][0]["capabilities"].is_null()
    });
    assert!(!reconnected["trusted_devices"][0]["capabilities"].is_null());
    inviter.shutdown();
    restarted.shutdown();
}

/// Lines of one conversation, one per 4 s, as both devices' live transcripts heard it.
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

/// Answers every window with "testo combinato" and counts the windows.
struct CountingEngine(Arc<std::sync::atomic::AtomicUsize>);

impl crate::ports::ai::TranscriptionEngine for CountingEngine {
    fn transcribe(
        &self,
        audio_recording: &crate::domain::models::AudioRecording,
        _context: &crate::domain::models::TranscriptionContext,
    ) -> Result<crate::domain::models::Transcript, TranscriptionError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(crate::domain::models::Transcript::new(
            audio_recording.meeting_id().clone(),
            "testo combinato",
        )
        .with_timed_words(vec![
            crate::domain::models::TimedWord::new("testo", 0, 300),
            crate::domain::models::TimedWord::new("combinato", 300, 700),
        ]))
    }
}

/// One device of the end-to-end test: its database, sealed speech, worker and P2P link.
struct Device {
    database: std::path::PathBuf,
    device_id: DeviceId,
    store: Arc<crate::adapters::audio::retained_audio_store::RetainedAudioStore>,
    worker: crate::refinement_worker::RefinementWorker,
}

impl Device {
    /// Wires a device as `start_transcript_pass` does, with a fake engine. The real clock keeps
    /// each meeting waiting for its partner, as on real devices.
    fn new(
        database: &std::path::Path,
        name: &str,
        is_phone: bool,
        link: &Arc<P2pHandle>,
        windows: &Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        use crate::refinement_worker::{
            RefinementControl, RefinementWorker, RepositoryFactory, RetainedSpeechInbox,
        };
        let device_id = DeviceId::new(format!("device-{name}"));
        let store = Arc::new(
            crate::adapters::audio::retained_audio_store::RetainedAudioStore::new(
                crate::refinement_worker::retained_audio_directory(database),
                test_cipher(),
            ),
        );
        let opener = || -> RepositoryFactory {
            let database = database.to_path_buf();
            let device_id = device_id.clone();
            Box::new(move || {
                Ok(Box::new(open_note_repository(
                    &database,
                    device_id.clone(),
                    test_cipher(),
                )?) as Box<dyn NoteRepository>)
            })
        };
        let control = Arc::new(RefinementControl::new(is_phone));
        if is_phone {
            control.set_charging(true);
        }
        link.accept_speech_with(Box::new(RetainedSpeechInbox::new(
            Arc::clone(&store),
            opener(),
            device_id.clone(),
            Arc::clone(&control),
        )));
        control.set_speech_sender(Arc::clone(link) as Arc<dyn SpeechSender>);
        let windows = Arc::clone(windows);
        let worker = RefinementWorker::new(
            Arc::clone(&store),
            control,
            Box::new(move |_audio| {
                Ok(Box::new(CountingEngine(Arc::clone(&windows)))
                    as Box<dyn crate::ports::ai::TranscriptionEngine>)
            }),
            opener(),
            device_id.clone(),
        );
        Self {
            database: database.to_path_buf(),
            device_id,
            store,
            worker,
        }
    }

    fn repository(&self) -> crate::adapters::storage::sqlite_adapter::SqliteNoteRepository {
        open_note_repository(&self.database, self.device_id.clone(), test_cipher())
            .unwrap_or_else(|error| panic!("{error:#}"))
    }

    /// Records `meeting` here: its live transcript (the talk heard `first_ms` in) and one second
    /// of sealed speech.
    fn record(&self, meeting: &str, first_ms: u64) {
        let meeting = MeetingId::new(meeting);
        let repository = self.repository();
        repository
            .create_meeting(&meeting, &automerge::AutoCommit::new().save())
            .unwrap_or_else(|error| panic!("{error}"));
        let segments: Vec<TranscriptSegment> = TALK
            .iter()
            .enumerate()
            .map(|(index, line)| {
                let start = first_ms + index as u64 * 4_000;
                TranscriptSegment::new(meeting.clone(), start, start + 3_800, None, *line)
            })
            .collect();
        repository
            .save_transcript_segments(&meeting, &segments)
            .unwrap_or_else(|error| panic!("{error}"));
        self.store
            .append_chunk(
                &meeting,
                crate::application::transcript_refinement::ChunkSpan {
                    start_ms: 0,
                    end_ms: 1_000,
                },
                None,
                &[7; 16_000],
            )
            .unwrap_or_else(|error| panic!("{error}"));
    }
}

/// End to end (#88): a laptop and a phone, paired over real P2P tasks, each record the same
/// conversation. Their meetings sync, both devices fold the phone's recording into the laptop's,
/// the phone sends its sealed speech, and the laptop transcribes both devices' speech in one pass.
/// Only Whisper is faked.
#[test]
fn a_meeting_recorded_on_laptop_and_phone_becomes_one_transcript() {
    let laptop_database = fresh_database("laptop");
    let phone_database = fresh_database("phone");
    let (laptop_task, phone_task) = pair_tasks(&laptop_database, &phone_database);
    let (laptop_task, phone_task) = (Arc::new(laptop_task), Arc::new(phone_task));
    let windows = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut laptop = Device::new(&laptop_database, "laptop", false, &laptop_task, &windows);
    let mut phone = Device::new(&phone_database, "phone", true, &phone_task, &windows);

    laptop.record("meeting-laptop", 3_000);
    phone.record("meeting-phone", 1_000);
    let laptop_meeting = MeetingId::new("meeting-laptop");
    let phone_meeting = MeetingId::new("meeting-phone");
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let mut rounds = 0;
    let done = loop {
        rounds += 1;
        phone.worker.run_pending();
        laptop.worker.run_pending();
        let finished = laptop.store.pending_meetings().is_empty()
            && phone.store.pending_meetings().is_empty()
            && windows.load(std::sync::atomic::Ordering::SeqCst) > 0;
        if finished || std::time::Instant::now() > deadline {
            break finished;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let laptop_recordings = laptop
        .repository()
        .meeting_recordings()
        .unwrap_or_else(|error| panic!("{error}"));
    let phone_recordings = phone
        .repository()
        .meeting_recordings()
        .unwrap_or_else(|error| panic!("{error}"));
    let laptop_text: Vec<String> = laptop
        .repository()
        .load_transcript_segments(&laptop_meeting)
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(|segment| segment.text().to_owned())
        .collect();
    laptop_task.shutdown();
    phone_task.shutdown();

    assert!(
        done,
        "the speech was not combined within a minute ({rounds} rounds)"
    );
    assert_eq!(
        windows.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "one window from each device, transcribed on the laptop"
    );
    assert!(
        laptop_text.iter().any(|text| text.contains("combinato")),
        "{laptop_text:?}"
    );
    for recordings in [&laptop_recordings, &phone_recordings] {
        let listing = crate::application::combined_meetings::MeetingListing::of(recordings);
        assert!(listing.is_listed(&laptop_meeting));
        assert!(
            !listing.is_listed(&phone_meeting),
            "listed once on both devices"
        );
        assert_eq!(
            listing.recorded_on(&laptop_meeting),
            vec![
                DeviceId::new("device-laptop"),
                DeviceId::new("device-phone")
            ]
        );
    }
    let fold = phone_recordings
        .iter()
        .find(|recording| recording.meeting_id == phone_meeting)
        .and_then(|recording| recording.combined_into.clone())
        .unwrap_or_else(|| panic!("the phone's recording must be folded"));
    assert_eq!(fold.combine_on, DeviceId::new("device-laptop"));
}

/// Renaming saves the name, shows it at once, and keeps it for the next start (#121).
#[test]
fn renaming_saves_the_name_and_the_task_uses_it_at_once_and_after_a_restart() {
    let folder = std::env::temp_dir().join(format!("ffi-p2p-rename-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let database_path = folder.join("nt.sqlite3");
    let handle = start_task_at(&database_path, "Laptop", &[]);

    let refused = json(&p2p_command_json_at(
        Some(&handle as &dyn P2pControl),
        Some(&database_path),
        r#"{"command":"set_device_name","name":"   "}"#,
    ));
    assert_eq!(refused["error_code"], "invalid_device_name");
    let renamed = json(&p2p_command_json_at(
        Some(&handle as &dyn P2pControl),
        Some(&database_path),
        r#"{"command":"set_device_name","name":" Studio laptop "}"#,
    ));
    assert_eq!(renamed["ok"], true);
    assert_eq!(renamed["device_name"], "Studio laptop");
    let status = wait_for_status(&handle, |status| status["device_name"] == "Studio laptop");
    assert_eq!(status["device_name"], "Studio laptop");
    drop(handle);

    let restarted = start_task_at(&database_path, "Laptop", &[]);
    let status = wait_for_status(&restarted, |status| {
        status["device_name"] == "Studio laptop"
    });
    assert_eq!(
        status["device_name"], "Studio laptop",
        "the saved name wins"
    );
    drop(restarted);
    drop(std::fs::remove_dir_all(folder));
}

/// Without sync running the name is still saved, for the next start.
#[test]
fn a_name_is_saved_even_while_sync_is_off() {
    let folder = std::env::temp_dir().join(format!("ffi-p2p-rename-off-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let database_path = folder.join("nt.sqlite3");

    let renamed = json(&p2p_command_json_at(
        None,
        Some(&database_path),
        r#"{"command":"set_device_name","name":"Phone"}"#,
    ));

    assert_eq!(renamed["ok"], true);
    assert_eq!(
        crate::device_name_settings::load_saved_device_name(&database_path)
            .map(|name| name.as_str().to_owned()),
        Some(String::from("Phone"))
    );
    drop(std::fs::remove_dir_all(folder));
}
