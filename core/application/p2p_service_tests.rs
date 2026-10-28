//! Specifies the P2P service that the daemon's task drives (Issue #43).
//!
//! Two services run over real libp2p swarms on the in-process memory transport, each with a
//! document-backed SQLite repository and an in-memory allowlist. The owner's steps are issued as
//! `P2pCommand`s, as the HTTP routes will issue them, and the tests check what the status snapshot
//! shows and what reaches the other device's repository.

use std::{path::PathBuf, sync::Arc, time::Duration};

use libp2p::Multiaddr;
use uuid::Uuid;

use crate::{
    adapters::{
        network::libp2p_adapter::Libp2pNetworkAdapter,
        storage::{sqlite_adapter::SqliteNoteRepository, sqlite_peer_store::SqlitePeerStore},
    },
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor, ephemeral_cipher},
        meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
        p2p_service::{P2pCommand, P2pCommandOutcome, P2pService},
        pairing_manager::PairingManager,
        repository_sync_manager::RepositorySyncManager,
    },
    domain::{
        models::{DeviceId, MeetingId, MeetingTitle, Transcript},
        peer_trust::DeviceName,
    },
    ports::{peer_trust::PairingTransport, storage::NoteRepository},
};

/// A service over the production adapter and stores.
type Service = P2pService<Libp2pNetworkAdapter, SqlitePeerStore, SqliteNoteRepository>;

/// Builds a service named `name` over `adapter`, with a fresh document-backed repository.
fn service(name: &str, adapter: Libp2pNetworkAdapter) -> Service {
    let directory = std::env::temp_dir().join(format!("p2p-service-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    let database_path: PathBuf = directory.join("notes.sqlite3");
    let repository = SqliteNoteRepository::open(
        &database_path,
        DeviceId::new(name),
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
    )
    .and_then(|repository| repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec)))
    .unwrap_or_else(|error| panic!("repository must open: {error}"));
    let trust_store = SqlitePeerStore::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("peer store must open: {error}"));
    let device_name =
        DeviceName::parse(name).unwrap_or_else(|error| panic!("valid device name: {error}"));
    let mut service = P2pService::new(
        PairingManager::new(adapter, trust_store, device_name, Box::new(|| 7)),
        RepositorySyncManager::new(repository),
    )
    .with_retry_delay(Duration::from_millis(50));
    service
        .start()
        .unwrap_or_else(|error| panic!("service must start: {error}"));
    service
}

/// Drives both swarms and both services until `condition` holds (5 s limit).
async fn drive(
    laptop: &mut Service,
    phone: &mut Service,
    mut condition: impl FnMut(&Service, &Service) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            laptop.process_pending_events();
            phone.process_pending_events();
            if condition(laptop, phone) {
                return;
            }
            // Wakes for scheduled retries too, as the daemon loop does.
            let next_retry = [laptop.next_retry_at(), phone.next_retry_at()]
                .into_iter()
                .flatten()
                .min();
            tokio::select! {
                result = laptop.transport_mut().drive_next_swarm_event() => {
                    result.unwrap_or_else(|error| panic!("laptop swarm: {error}"));
                }
                result = phone.transport_mut().drive_next_swarm_event() => {
                    result.unwrap_or_else(|error| panic!("phone swarm: {error}"));
                }
                () = sleep_until_retry(next_retry) => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("condition not reached before timeout"));
}

/// Sleeps until `retry_at`, or forever when no retry is scheduled.
async fn sleep_until_retry(retry_at: Option<std::time::Instant>) {
    match retry_at {
        Some(retry_at) => tokio::time::sleep_until(tokio::time::Instant::from_std(retry_at)).await,
        None => std::future::pending().await,
    }
}

/// Runs one command and fails the test on error.
fn run(service: &mut Service, command: P2pCommand) -> P2pCommandOutcome {
    service
        .handle_command(command)
        .unwrap_or_else(|error| panic!("command must succeed: {error}"))
}

/// Reads the projected title of `meeting_id` on `service`'s device, if the meeting exists.
fn stored_title(service: &Service, meeting_id: &MeetingId) -> Option<String> {
    service
        .repository()
        .load_meeting_metadata(meeting_id)
        .unwrap_or_else(|error| panic!("metadata must load: {error}"))
        .and_then(|metadata| metadata.title().map(|title| title.as_str().to_owned()))
}

/// Parses a fixture title.
fn title(text: &str) -> MeetingTitle {
    MeetingTitle::parse(text).unwrap_or_else(|error| panic!("title must parse: {error}"))
}

/// The owner pairs a laptop and a phone through commands. Afterwards the laptop's meeting reaches
/// the phone, a later rename follows, and revocation removes the phone from the laptop's devices.
#[tokio::test]
async fn paired_devices_sync_meetings_and_later_changes() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9351"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let mut laptop = service("Laptop", laptop_adapter);
    let mut phone = service("Phone", phone_adapter);
    let laptop_id = laptop.transport().local_peer_id();
    let phone_id = phone.transport().local_peer_id();
    let meeting_id = MeetingId::new("meeting-standup");
    MeetingCrdtService::new(laptop.repository())
        .create_titled_meeting(&meeting_id, &title("Standup"))
        .and_then(|()| {
            laptop
                .repository()
                .save_transcript(&Transcript::new(meeting_id.clone(), "Ciao Hello"))
        })
        .unwrap_or_else(|error| panic!("meeting must be recorded: {error}"));

    // The phone becomes pairable; the laptop connects (mDNS supplies the address in production).
    run(&mut phone, P2pCommand::MakePairable);
    laptop
        .transport_mut()
        .allow_pairing_dial(&phone_id)
        .and_then(|()| laptop.transport_mut().dial_memory(phone_address))
        .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
    drive(&mut laptop, &mut phone, |laptop, _| {
        laptop.transport().is_connected_to(&phone_id)
    })
    .await;
    let P2pCommandOutcome::PairingStarted(laptop_code) =
        run(&mut laptop, P2pCommand::StartPairing(phone_id.clone()))
    else {
        panic!("starting a pairing must return its code");
    };
    drive(&mut laptop, &mut phone, |_, phone| {
        !phone.status().incoming_requests.is_empty()
    })
    .await;
    let phone_status = phone.status();
    assert_eq!(phone_status.incoming_requests[0].peer_id, laptop_id);
    assert_eq!(phone_status.incoming_requests[0].code, laptop_code);
    assert_eq!(laptop.status().outgoing_pairings[0].code, laptop_code);

    // The owner sees matching codes and confirms on both devices.
    run(&mut laptop, P2pCommand::ConfirmCode(phone_id.clone()));
    run(&mut phone, P2pCommand::AcceptIncoming(laptop_id.clone()));
    drive(&mut laptop, &mut phone, |laptop, phone| {
        !laptop.status().trusted_devices.is_empty() && stored_title(phone, &meeting_id).is_some()
    })
    .await;
    let laptop_status = laptop.status();
    assert_eq!(laptop_status.trusted_devices[0].peer_id(), &phone_id);
    assert!(laptop_status.outgoing_pairings.is_empty());
    assert_eq!(phone.status().trusted_devices[0].peer_id(), &laptop_id);
    assert_eq!(
        stored_title(&phone, &meeting_id).as_deref(),
        Some("Standup")
    );
    let synced_transcript = phone
        .repository()
        .load_transcript(&meeting_id)
        .unwrap_or_else(|error| panic!("transcript must load: {error}"));
    assert_eq!(
        synced_transcript,
        Some(Transcript::new(meeting_id.clone(), "Ciao Hello"))
    );

    // A rename after pairing reaches the phone once the laptop reports the local change.
    MeetingCrdtService::new(laptop.repository())
        .set_meeting_title(&meeting_id, &title("Weekly standup"))
        .unwrap_or_else(|error| panic!("rename must succeed: {error}"));
    run(&mut laptop, P2pCommand::LocalChange(meeting_id.clone()));
    drive(&mut laptop, &mut phone, |_, phone| {
        stored_title(phone, &meeting_id).as_deref() == Some("Weekly standup")
    })
    .await;

    // Revocation removes the phone from the laptop's paired devices.
    assert_eq!(
        run(&mut laptop, P2pCommand::Revoke(phone_id.clone())),
        P2pCommandOutcome::Revoked(true)
    );
    assert!(laptop.status().trusted_devices.is_empty());
}

/// The snapshot names this device, and a command for a pairing that does not exist fails without
/// changing anything.
#[tokio::test]
async fn status_names_the_device_and_unknown_pairings_are_refused() {
    let adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut laptop = service("Laptop", adapter);

    let status = laptop.status();
    let refused = laptop.handle_command(P2pCommand::AcceptIncoming(DeviceId::new("peer-x")));

    assert_eq!(status.local_peer_id, laptop.transport().local_peer_id());
    assert_eq!(status.device_name.as_str(), "Laptop");
    assert!(status.trusted_devices.is_empty() && status.incoming_requests.is_empty());
    assert!(refused.is_err());
}

/// The phone accepts first and syncs its meeting at once, while the laptop, which has not
/// confirmed yet, still drops those messages. Once the laptop confirms, the phone's retry
/// delivers the meeting.
#[tokio::test]
async fn messages_dropped_before_mutual_trust_are_sent_again() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9352"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let mut laptop = service("Laptop", laptop_adapter);
    let mut phone = service("Phone", phone_adapter);
    let laptop_id = laptop.transport().local_peer_id();
    let phone_id = phone.transport().local_peer_id();
    let meeting_id = MeetingId::new("meeting-on-phone");
    MeetingCrdtService::new(phone.repository())
        .create_titled_meeting(&meeting_id, &title("Phone call"))
        .unwrap_or_else(|error| panic!("meeting must be recorded: {error}"));
    run(&mut phone, P2pCommand::MakePairable);
    laptop
        .transport_mut()
        .allow_pairing_dial(&phone_id)
        .and_then(|()| laptop.transport_mut().dial_memory(phone_address))
        .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
    drive(&mut laptop, &mut phone, |laptop, _| {
        laptop.transport().is_connected_to(&phone_id)
    })
    .await;
    run(&mut laptop, P2pCommand::StartPairing(phone_id.clone()));
    drive(&mut laptop, &mut phone, |_, phone| {
        !phone.status().incoming_requests.is_empty()
    })
    .await;

    // The phone accepts and starts syncing before the laptop's owner has confirmed.
    run(&mut phone, P2pCommand::AcceptIncoming(laptop_id));
    drive(&mut laptop, &mut phone, |_, phone| {
        phone.next_retry_at().is_some()
    })
    .await;
    assert_eq!(stored_title(&laptop, &meeting_id), None);

    run(&mut laptop, P2pCommand::ConfirmCode(phone_id));
    drive(&mut laptop, &mut phone, |laptop, _| {
        stored_title(laptop, &meeting_id).is_some()
    })
    .await;
    assert_eq!(
        stored_title(&laptop, &meeting_id).as_deref(),
        Some("Phone call")
    );
}
