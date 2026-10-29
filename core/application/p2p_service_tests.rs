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
        crypto::{
            pair_key::PairAgreementSecret,
            voice_encryptor::{EncryptionKey, VoiceEncryptor},
        },
        meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
        p2p_service::{LocalCapabilities, P2pCommand, P2pCommandOutcome, P2pService},
        pairing_manager::PairingManager,
        repository_sync_manager::RepositorySyncManager,
    },
    domain::{
        models::{DeviceId, MeetingId, MeetingTitle, Transcript, TranscriptionModel},
        peer_trust::DeviceName,
    },
    ports::{
        peer_trust::{PairingTransport, TrustedPeerStore},
        storage::NoteRepository,
    },
};

/// A service over the production adapter and stores.
type Service = P2pService<Libp2pNetworkAdapter, SqlitePeerStore, SqliteNoteRepository>;

/// Builds a service named `name` over `adapter`, with a fresh document-backed repository.
fn service(name: &str, adapter: Libp2pNetworkAdapter) -> Service {
    service_with_database(name, adapter).0
}

/// Opens another connection to a service's database, as the recording pipeline does.
fn pipeline_connection(database_path: &PathBuf, name: &str) -> SqliteNoteRepository {
    SqliteNoteRepository::open(
        database_path,
        DeviceId::new(name),
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
    )
    .and_then(|repository| repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec)))
    .unwrap_or_else(|error| panic!("pipeline connection must open: {error}"))
}

/// Builds a service like `service` and also returns its database path.
fn service_with_database(name: &str, adapter: Libp2pNetworkAdapter) -> (Service, PathBuf) {
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
    // The allowlist shares the database file, as in production, so a second connection can
    // change it the way the CLI does.
    let trust_store = trust_store_connection(&database_path);
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
    (service, database_path)
}

/// Opens another connection to a service's allowlist, as the CLI's `devices` commands do.
fn trust_store_connection(database_path: &PathBuf) -> SqlitePeerStore {
    SqlitePeerStore::open(
        database_path,
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
    )
    .unwrap_or_else(|error| panic!("peer store must open: {error}"))
}

/// Drives both swarms and both services until `condition` holds (5 s limit).
async fn drive(
    laptop: &mut Service,
    phone: &mut Service,
    mut condition: impl FnMut(&Service, &Service) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            laptop.poll_local_changes();
            phone.poll_local_changes();
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

/// Connects `dialer` to `listener` as already-paired devices, listening on `memory_address`.
fn link(dialer: &mut Service, listener: &mut Service, memory_address: &str) {
    let address: Multiaddr = memory_address
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    let dialer_id = dialer.transport().local_peer_id();
    let listener_id = listener.transport().local_peer_id();
    listener
        .transport_mut()
        .listen_on_memory(address.clone())
        .and_then(|()| listener.transport_mut().trust_peer(&dialer_id))
        .and_then(|()| dialer.transport_mut().trust_peer(&listener_id))
        .and_then(|()| dialer.transport_mut().dial_memory(address))
        .unwrap_or_else(|error| panic!("devices must link: {error}"));
}

/// Drives every service, as each daemon's loop would, until `condition` holds (5 s limit).
async fn drive_all(
    services: &mut [&mut Service],
    mut condition: impl FnMut(&[&mut Service]) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            for service in services.iter_mut() {
                service.poll_local_changes();
                service.process_pending_events();
            }
            if condition(services) {
                return;
            }
            for service in services.iter_mut() {
                let _ = tokio::time::timeout(
                    Duration::from_millis(5),
                    service.transport_mut().drive_next_swarm_event(),
                )
                .await;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("condition not reached before timeout"));
}

/// Reads the transcript of `meeting_id` on `service`'s device.
fn stored_transcript(service: &Service, meeting_id: &MeetingId) -> Option<String> {
    service
        .repository()
        .load_transcript(meeting_id)
        .ok()
        .flatten()
        .map(|transcript| transcript.content().to_owned())
}

/// A transcript the recording pipeline saves through its own connection reaches the connected
/// phone right away, without a reconnect.
#[tokio::test]
async fn a_transcript_saved_by_the_pipeline_reaches_a_connected_device_at_once() {
    let (mut laptop, laptop_database) = service_with_database(
        "Laptop",
        Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}")),
    );
    let mut phone = service(
        "Phone",
        Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}")),
    );
    link(&mut laptop, &mut phone, "/memory/9371");
    let phone_id = phone.transport().local_peer_id();
    drive_all(&mut [&mut laptop, &mut phone], |services| {
        services[0].transport().is_connected_to(&phone_id)
    })
    .await;

    let meeting_id = MeetingId::new("meeting-live");
    let pipeline = pipeline_connection(&laptop_database, "Laptop");
    MeetingCrdtService::new(&pipeline)
        .create_titled_meeting(&meeting_id, &title("Live recording"))
        .and_then(|()| pipeline.save_transcript(&Transcript::new(meeting_id.clone(), "Ciao")))
        .unwrap_or_else(|error| panic!("pipeline must save: {error}"));
    drive_all(&mut [&mut laptop, &mut phone], |services| {
        stored_transcript(services[1], &meeting_id).is_some()
    })
    .await;
    pipeline
        .save_transcript(&Transcript::new(meeting_id.clone(), "Ciao a tutti"))
        .unwrap_or_else(|error| panic!("pipeline must save: {error}"));
    drive_all(&mut [&mut laptop, &mut phone], |services| {
        stored_transcript(services[1], &meeting_id).as_deref() == Some("Ciao a tutti")
    })
    .await;

    assert_eq!(
        stored_title(&phone, &meeting_id).as_deref(),
        Some("Live recording")
    );
}

/// A device connected only to a middle device still gets a new transcript at once: the middle
/// device forwards what it receives.
#[tokio::test]
async fn changes_are_forwarded_to_devices_connected_only_through_another() {
    let adapter = || {
        Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"))
    };
    let (mut laptop, laptop_database) = service_with_database("Laptop", adapter());
    let mut desktop = service("Desktop", adapter());
    let mut phone = service("Phone", adapter());
    link(&mut laptop, &mut desktop, "/memory/9372");
    link(&mut phone, &mut desktop, "/memory/9373");
    let desktop_id = desktop.transport().local_peer_id();
    drive_all(&mut [&mut laptop, &mut desktop, &mut phone], |services| {
        services[0].transport().is_connected_to(&desktop_id)
            && services[2].transport().is_connected_to(&desktop_id)
    })
    .await;

    let meeting_id = MeetingId::new("meeting-relayed");
    let pipeline = pipeline_connection(&laptop_database, "Laptop");
    MeetingCrdtService::new(&pipeline)
        .create_titled_meeting(&meeting_id, &title("Relayed"))
        .and_then(|()| pipeline.save_transcript(&Transcript::new(meeting_id.clone(), "Ciao")))
        .unwrap_or_else(|error| panic!("pipeline must save: {error}"));
    drive_all(&mut [&mut laptop, &mut desktop, &mut phone], |services| {
        stored_transcript(services[2], &meeting_id).is_some()
    })
    .await;

    assert_eq!(
        stored_title(&phone, &meeting_id).as_deref(),
        Some("Relayed")
    );
}

/// A device that was offline while a meeting was recorded gets it when it comes online, from a
/// device that is online, even though the recorder itself is offline by then.
#[tokio::test]
async fn a_device_coming_online_later_gets_meetings_from_whoever_is_online() {
    let adapter = || {
        Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"))
    };
    let (mut laptop, laptop_database) = service_with_database("Laptop", adapter());
    let mut desktop = service("Desktop", adapter());
    link(&mut laptop, &mut desktop, "/memory/9374");
    let meeting_id = MeetingId::new("meeting-while-phone-offline");
    let pipeline = pipeline_connection(&laptop_database, "Laptop");
    MeetingCrdtService::new(&pipeline)
        .create_titled_meeting(&meeting_id, &title("Phone was off"))
        .and_then(|()| pipeline.save_transcript(&Transcript::new(meeting_id.clone(), "Ciao")))
        .unwrap_or_else(|error| panic!("pipeline must save: {error}"));
    drive_all(&mut [&mut laptop, &mut desktop], |services| {
        stored_transcript(services[1], &meeting_id).is_some()
    })
    .await;
    // The laptop goes offline; then the phone comes online and reaches only the desktop.
    drop(laptop);
    let mut phone = service("Phone", adapter());
    link(&mut phone, &mut desktop, "/memory/9375");
    drive_all(&mut [&mut desktop, &mut phone], |services| {
        stored_transcript(services[1], &meeting_id).is_some()
    })
    .await;

    assert_eq!(
        stored_transcript(&phone, &meeting_id).as_deref(),
        Some("Ciao")
    );
    assert_eq!(
        stored_title(&phone, &meeting_id).as_deref(),
        Some("Phone was off")
    );
}

/// Pairs `laptop` with `phone` (listening on `phone_address`), confirming the code on both, and
/// drives them until each lists the other as paired.
async fn pair(laptop: &mut Service, phone: &mut Service, phone_address: Multiaddr) {
    let laptop_id = laptop.transport().local_peer_id();
    let phone_id = phone.transport().local_peer_id();
    run(phone, P2pCommand::MakePairable);
    laptop
        .transport_mut()
        .allow_pairing_dial(&phone_id)
        .and_then(|()| laptop.transport_mut().dial_memory(phone_address))
        .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
    drive(laptop, phone, |laptop, _| {
        laptop.transport().is_connected_to(&phone_id)
    })
    .await;
    run(laptop, P2pCommand::StartPairing(phone_id.clone()));
    drive(laptop, phone, |_, phone| {
        !phone.status().incoming_requests.is_empty()
    })
    .await;
    run(laptop, P2pCommand::ConfirmCode(phone_id));
    run(phone, P2pCommand::AcceptIncoming(laptop_id));
    drive(laptop, phone, |laptop, phone| {
        !laptop.status().trusted_devices.is_empty() && !phone.status().trusted_devices.is_empty()
    })
    .await;
}

/// A device removed from the allowlist by another process (the CLI's `devices revoke`) is dropped
/// by the running service on its next trust poll: it leaves the paired devices and is
/// disconnected, without a restart.
#[tokio::test]
async fn a_revocation_made_by_another_process_takes_effect_on_the_next_poll() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9377"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (mut laptop, laptop_database) = service_with_database("Laptop", laptop_adapter);
    let mut phone = service("Phone", phone_adapter);
    let phone_id = phone.transport().local_peer_id();
    pair(&mut laptop, &mut phone, phone_address).await;
    laptop.poll_trust_changes();
    assert_eq!(
        laptop.status().trusted_devices.len(),
        1,
        "nothing changed yet"
    );

    let was_trusted = trust_store_connection(&laptop_database)
        .revoke_peer(&phone_id)
        .unwrap_or_else(|error| panic!("revoke must succeed: {error}"));
    laptop.poll_trust_changes();

    assert!(was_trusted);
    assert!(laptop.status().trusted_devices.is_empty());
    drive(&mut laptop, &mut phone, |laptop, _| {
        !laptop.transport().is_connected_to(&phone_id)
    })
    .await;
}

/// Paired devices exchange their manifests once connected (REQUIREMENTS 3.4): each status shows
/// the other's capabilities under its peer ID, and a revoked device's manifest is forgotten.
#[tokio::test]
async fn paired_devices_exchange_manifests_and_forget_revoked_ones() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9401"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let mut laptop = service("Laptop", laptop_adapter).with_local_capabilities(LocalCapabilities {
        cpu_cores: 8,
        gpu_vram_megabytes: 4_096,
        transcription_models: vec![TranscriptionModel::Whisper],
    });
    let mut phone = service("Phone", phone_adapter).with_local_capabilities(LocalCapabilities {
        cpu_cores: 4,
        gpu_vram_megabytes: 0,
        transcription_models: Vec::new(),
    });
    let laptop_id = laptop.transport().local_peer_id();
    let phone_id = phone.transport().local_peer_id();
    assert_eq!(laptop.status().local_manifest.cpu_cores(), 8);
    assert_eq!(laptop.status().local_manifest.device_id(), &laptop_id);

    pair(&mut laptop, &mut phone, phone_address).await;
    drive(&mut laptop, &mut phone, |laptop, phone| {
        !laptop.status().peer_manifests.is_empty() && !phone.status().peer_manifests.is_empty()
    })
    .await;

    let phone_manifest_on_laptop = &laptop.status().peer_manifests[0];
    assert_eq!(phone_manifest_on_laptop.device_id(), &phone_id);
    assert_eq!(phone_manifest_on_laptop.cpu_cores(), 4);
    assert!(
        phone_manifest_on_laptop
            .available_transcription_models()
            .is_empty()
    );
    let laptop_manifest_on_phone = &phone.status().peer_manifests[0];
    assert_eq!(laptop_manifest_on_phone.device_id(), &laptop_id);
    assert_eq!(laptop_manifest_on_phone.gpu_vram_megabytes(), 4_096);
    assert!(laptop_manifest_on_phone.supports_transcription_model(&TranscriptionModel::Whisper));

    run(&mut laptop, P2pCommand::Revoke(phone_id));
    laptop.process_pending_events();
    assert!(laptop.status().peer_manifests.is_empty());
}

/// Paired devices agree a per-pair key once connected (Issue #50): each sends its X25519 public
/// key, and both allowlists end up holding the same key for the other device. Devices paired
/// before #50 get theirs the same way, at their next connection.
#[tokio::test]
async fn paired_devices_agree_the_same_pair_key() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9411"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (laptop, laptop_database) = service_with_database("Laptop", laptop_adapter);
    let (phone, phone_database) = service_with_database("Phone", phone_adapter);
    let mut laptop = laptop.with_pair_agreement_secret(PairAgreementSecret::generate());
    let mut phone = phone.with_pair_agreement_secret(PairAgreementSecret::generate());
    let laptop_id = laptop.transport().local_peer_id();
    let phone_id = phone.transport().local_peer_id();
    let laptop_store = trust_store_connection(&laptop_database);
    let phone_store = trust_store_connection(&phone_database);

    pair(&mut laptop, &mut phone, phone_address).await;
    drive(&mut laptop, &mut phone, |_, _| {
        matches!(laptop_store.pair_key(&phone_id), Ok(Some(_)))
            && matches!(phone_store.pair_key(&laptop_id), Ok(Some(_)))
    })
    .await;

    let on_laptop = laptop_store
        .pair_key(&phone_id)
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("laptop must hold the pair key"));
    let on_phone = phone_store
        .pair_key(&laptop_id)
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("phone must hold the pair key"));
    assert!(on_laptop.matches(&on_phone));
}
