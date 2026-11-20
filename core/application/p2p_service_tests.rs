//! Specifies the P2P service that the daemon's task drives (Issue #43).
//!
//! Two services run over real libp2p swarms on the in-process memory transport, each with a
//! document-backed SQLite repository and an in-memory allowlist. The owner's steps are issued as
//! `P2pCommand`s, as the HTTP routes will issue them, and the tests check what the status snapshot
//! shows and what reaches the other device's repository.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use libp2p::Multiaddr;
use uuid::Uuid;

use crate::{
    adapters::{
        network::libp2p_adapter::Libp2pNetworkAdapter,
        storage::{
            attachment_file_store::AttachmentFileStore, attachment_sync_store::AttachmentSyncStore,
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
            sqlite_peer_store::SqlitePeerStore, sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::{
        cluster_review::assign_cluster_to_named_identity,
        compute_router::ComputePreference,
        crypto::{
            pair_key::PairAgreementSecret,
            voice_encryptor::{EncryptionKey, VoiceEncryptor},
        },
        meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
        p2p_service::{
            LocalCapabilities, P2pCommand, P2pCommandOutcome, P2pService, RemoteTranscriptionJob,
            SpeechTransferJob,
        },
        pairing_manager::PairingManager,
        repository_sync_manager::RepositorySyncManager,
    },
    domain::{
        errors::TranscriptionError,
        models::{
            Attachment, DeviceId, MeetingId, MeetingTitle, SpeakerIdentity, Transcript,
            TranscriptionModel, UnassignedVoiceClip, UnknownSpeakerChunk, UnknownSpeakerCluster,
            VoiceEmbedding,
        },
        peer_trust::DeviceName,
        projects::ProjectName,
    },
    ports::{
        peer_trust::{PairingTransport, TrustedPeerStore},
        projects::ProjectRepository,
        remote_transcription::{FinishedTranscription, HostedTranscription, TranscriptionHost},
        speech_transfer::{SpeechChunk, SpeechInbox, SpeechPart, SpeechTransferError},
        storage::{AttachmentRepository, NoteRepository},
        voice::VoiceVectorDB,
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

/// How long a drive may take before the test fails. The drives end as soon as their condition holds,
/// so this only stops a hang: it is generous because on a busy machine a debug build moving an
/// attachment through SQLite can take several seconds (#81).
const DRIVE_DEADLINE: Duration = Duration::from_secs(60);

/// Drives both swarms and both services until `condition` holds, failing after `DRIVE_DEADLINE`.
async fn drive(
    laptop: &mut Service,
    phone: &mut Service,
    mut condition: impl FnMut(&Service, &Service) -> bool,
) {
    tokio::time::timeout(DRIVE_DEADLINE, async {
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

/// Renaming changes the name the status shows at once (#121).
#[tokio::test]
async fn renaming_changes_the_announced_name() {
    let adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut laptop = service("Laptop", adapter);
    let new_name = crate::domain::peer_trust::DeviceName::parse("Studio laptop")
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        run(&mut laptop, P2pCommand::Rename(new_name)),
        P2pCommandOutcome::Done
    );

    assert_eq!(laptop.status().device_name.as_str(), "Studio laptop");
    assert!(laptop.status().nearby_device_names.is_empty());
}

/// A device renamed after pairing shows its new name on the paired device too (#121).
#[tokio::test]
async fn a_rename_after_pairing_reaches_the_paired_device() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9353"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let mut laptop = service("Laptop", laptop_adapter);
    let mut phone = service("Phone", phone_adapter);
    pair(&mut laptop, &mut phone, phone_address).await;
    let new_name = crate::domain::peer_trust::DeviceName::parse("S23")
        .unwrap_or_else(|error| panic!("{error}"));

    run(&mut phone, P2pCommand::Rename(new_name));
    drive(&mut laptop, &mut phone, |laptop, _| {
        laptop
            .status()
            .trusted_devices
            .iter()
            .any(|device| device.device_name().as_str() == "S23")
    })
    .await;

    assert_eq!(laptop.status().trusted_devices.len(), 1);
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

/// Drives every service, as each daemon's loop would, until `condition` holds, failing after
/// `DRIVE_DEADLINE`.
async fn drive_all(
    services: &mut [&mut Service],
    mut condition: impl FnMut(&[&mut Service]) -> bool,
) {
    tokio::time::timeout(DRIVE_DEADLINE, async {
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

/// The samples and context a host was given.
type ReceivedChunk = (Vec<i16>, Option<String>);

/// A host that answers from a script, or holds jobs when told to stay silent.
#[derive(Clone, Default)]
struct FakeHost {
    answer: Option<String>,
    silent: bool,
    finished: Arc<Mutex<Vec<FinishedTranscription>>>,
    received: Arc<Mutex<Vec<ReceivedChunk>>>,
}

impl TranscriptionHost for FakeHost {
    fn submit(&mut self, job: HostedTranscription) -> bool {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((job.samples, job.previous_text));
        if !self.silent {
            self.finished
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(FinishedTranscription {
                    peer: job.peer,
                    job_id: job.job_id,
                    text: self.answer.clone(),
                });
        }
        true
    }
    fn poll_finished(&mut self) -> Option<FinishedTranscription> {
        self.finished
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop()
    }
}

/// Pairs a phone and a laptop whose host is `host`, with agreed pair keys.
async fn paired_with_host(memory_address: &str, host: FakeHost) -> (Service, Service) {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = memory_address
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let mut laptop = service("Laptop", laptop_adapter)
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_transcription_host(Box::new(host));
    let (phone, phone_database) = service_with_database("Phone", phone_adapter);
    let mut phone = phone.with_pair_agreement_secret(PairAgreementSecret::generate());
    let phone_store = trust_store_connection(&phone_database);
    pair(&mut laptop, &mut phone, phone_address).await;
    let laptop_id = laptop.transport().local_peer_id();
    drive(&mut laptop, &mut phone, |_, phone| {
        phone.status().peer_manifests.len() == 1
            && phone_store.pair_key(&laptop_id).ok().flatten().is_some()
    })
    .await;
    (laptop, phone)
}

/// Sends a chunk from the phone to the laptop and returns where the answer arrives.
fn ask(
    phone: &mut Service,
    laptop: &Service,
) -> std::sync::mpsc::Receiver<Result<String, TranscriptionError>> {
    let (reply, answer) = std::sync::mpsc::channel();
    phone.request_remote_transcription(RemoteTranscriptionJob {
        peer: laptop.transport().local_peer_id(),
        samples: vec![1, -2, 3],
        previous_text: Some("before".to_owned()),
        language: None,
        reply,
    });
    answer
}

/// A chunk reaches the paired device with its context, and the text comes back.
#[tokio::test]
async fn a_paired_device_transcribes_a_chunk() {
    let host = FakeHost {
        answer: Some("words from the laptop".to_owned()),
        ..FakeHost::default()
    };
    let received = Arc::clone(&host.received);
    let (mut laptop, mut phone) = paired_with_host("/memory/9471", host).await;

    let answer = ask(&mut phone, &laptop);
    let mut result = None;
    drive(&mut laptop, &mut phone, |_, _| {
        result = answer.try_recv().ok();
        result.is_some()
    })
    .await;

    assert_eq!(
        result.map(|text| text.ok()),
        Some(Some("words from the laptop".to_owned()))
    );
    assert_eq!(
        *received.lock().unwrap_or_else(PoisonError::into_inner),
        vec![(vec![1, -2, 3], Some("before".to_owned()))]
    );
}

/// A device that cannot transcribe says so, and the phone hears it as a failure at once.
#[tokio::test]
async fn a_failed_hosted_job_is_reported() {
    let (mut laptop, mut phone) = paired_with_host("/memory/9472", FakeHost::default()).await;

    let answer = ask(&mut phone, &laptop);
    let mut result = None;
    drive(&mut laptop, &mut phone, |_, _| {
        result = answer.try_recv().ok();
        result.is_some()
    })
    .await;

    assert!(matches!(result, Some(Err(TranscriptionError::Unavailable))));
}

/// A device that disconnects while transcribing fails the wait without any timeout.
#[tokio::test]
async fn a_disconnection_mid_task_fails_the_wait_at_once() {
    let host = FakeHost {
        silent: true,
        ..FakeHost::default()
    };
    let received = Arc::clone(&host.received);
    let (mut laptop, mut phone) = paired_with_host("/memory/9473", host).await;
    let answer = ask(&mut phone, &laptop);
    drive(&mut laptop, &mut phone, |_, _| {
        !received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    })
    .await;

    let laptop_id = laptop.transport().local_peer_id();
    phone.transport_mut().disconnect_for_tests(&laptop_id);
    let mut result = None;
    drive(&mut laptop, &mut phone, |_, _| {
        result = answer.try_recv().ok();
        result.is_some()
    })
    .await;

    assert!(matches!(result, Some(Err(TranscriptionError::Unavailable))));
}

/// A device that is not connected gets nothing, and the failure is immediate.
#[tokio::test]
async fn a_chunk_for_an_absent_device_fails_at_once() {
    let adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone = service("Phone", adapter);
    let (reply, answer) = std::sync::mpsc::channel();

    phone.request_remote_transcription(RemoteTranscriptionJob {
        peer: DeviceId::new("12D3KooWAway"),
        samples: vec![1],
        previous_text: None,
        language: None,
        reply,
    });

    assert!(matches!(
        answer.try_recv(),
        Ok(Err(TranscriptionError::Unavailable))
    ));
}

/// The choice starts as Local, accepts only paired devices, shows in the status, and survives a
/// restart.
#[tokio::test]
async fn the_transcription_choice_is_validated_and_kept() {
    let adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let (phone, database) = service_with_database("Phone", adapter);
    let mut phone =
        phone.with_transcription_route_store(Box::new(trust_store_connection(&database)));
    assert_eq!(
        phone.status().transcription_route,
        ComputePreference::LocalOnly
    );

    assert!(
        phone
            .handle_command(P2pCommand::SetTranscriptionRoute(
                ComputePreference::SpecificPeer(DeviceId::new("12D3KooWStranger"))
            ))
            .is_err()
    );
    run(
        &mut phone,
        P2pCommand::SetTranscriptionRoute(ComputePreference::Automatic),
    );
    assert_eq!(
        phone.status().transcription_route,
        ComputePreference::Automatic
    );

    let adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let restarted = service("Phone", adapter)
        .with_transcription_route_store(Box::new(trust_store_connection(&database)));
    assert_eq!(
        restarted.status().transcription_route,
        ComputePreference::Automatic
    );
}

/// Reads a stored file's clear contents with the database's key (#99).
fn clear_contents(database_path: &PathBuf, file_path: &str) -> Option<Vec<u8>> {
    pipeline_connection(database_path, "Reader")
        .sealed_files()
        .read_all(std::path::Path::new(file_path))
        .ok()
        .map(|contents| contents.to_vec())
}

/// Adds a file to `meeting_id` through the same store the app uses.
fn attach_file(
    database_path: &PathBuf,
    meeting_id: &MeetingId,
    name: &str,
    contents: &[u8],
    meeting_time_ms: u64,
) -> Attachment {
    let directory = database_path
        .parent()
        .unwrap_or_else(|| panic!("the database has a folder"));
    let source = directory.join(name);
    std::fs::write(&source, contents)
        .unwrap_or_else(|error| panic!("source must be written: {error}"));
    let repository = pipeline_connection(database_path, "Laptop");
    AttachmentFileStore::new(
        directory.join("attachments"),
        DeviceId::new("Laptop"),
        repository.sealed_files(),
    )
    .import_file(&repository, meeting_id, &source, Some(meeting_time_ms))
    .unwrap_or_else(|error| panic!("the file must attach: {error}"))
}

/// A library over `database_path` keeping files in `attachments` beside it.
fn attachment_library(database_path: &PathBuf) -> Box<AttachmentSyncStore> {
    Box::new(AttachmentSyncStore::new(
        pipeline_connection(database_path, "sync"),
        database_path
            .parent()
            .unwrap_or_else(|| panic!("the database has a folder"))
            .join("attachments"),
    ))
}

/// The attachments of a meeting, as the app reads them.
fn attachments_of(database_path: &PathBuf, meeting_id: &MeetingId) -> Vec<Attachment> {
    pipeline_connection(database_path, "reader")
        .get_attachments_for_meeting(meeting_id)
        .unwrap_or_default()
}

/// An attachment added on one device reaches the other with its file and its place in the
/// transcript; one added while they are apart arrives after they reconnect.
#[tokio::test]
async fn attachments_reach_paired_devices_even_after_being_apart() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9457"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (laptop, laptop_database) = service_with_database("Laptop", laptop_adapter);
    let (phone, phone_database) = service_with_database("Phone", phone_adapter);
    let mut laptop = laptop
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_attachment_library(attachment_library(&laptop_database));
    let mut phone = phone
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_attachment_library(attachment_library(&phone_database));
    let meeting_id = MeetingId::new("meeting-attached");
    MeetingCrdtService::new(pipeline_connection(&laptop_database, "Laptop"))
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
    // Larger than one chunk, so the transfer takes several round trips.
    let photo: Vec<u8> = (0..200_000_u32)
        .map(|value| value.to_le_bytes()[0])
        .collect();
    let sent = attach_file(&laptop_database, &meeting_id, "board.png", &photo, 42_000);

    pair(&mut laptop, &mut phone, phone_address.clone()).await;
    drive(&mut laptop, &mut phone, |_, _| {
        !attachments_of(&phone_database, &meeting_id).is_empty()
    })
    .await;

    let received = attachments_of(&phone_database, &meeting_id);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].attachment_id(), sent.attachment_id());
    assert_eq!(received[0].meeting_time_ms(), Some(42_000));
    assert_eq!(received[0].mime_type(), "image/png");
    assert_ne!(received[0].file_path(), sent.file_path());
    assert_eq!(
        clear_contents(&phone_database, received[0].file_path()),
        Some(photo)
    );

    // Apart: the phone goes away and the laptop gets another attachment.
    let phone_id = phone.transport().local_peer_id();
    laptop.transport_mut().disconnect_for_tests(&phone_id);
    drive(&mut laptop, &mut phone, |laptop, _| {
        !laptop.transport().is_connected_to(&phone_id)
    })
    .await;
    let later = attach_file(&laptop_database, &meeting_id, "notes.txt", b"later", 50_000);
    for _ in 0..3 {
        laptop.poll_local_changes();
    }
    assert_eq!(attachments_of(&phone_database, &meeting_id).len(), 1);

    laptop
        .transport_mut()
        .dial_memory(phone_address)
        .unwrap_or_else(|error| panic!("the laptop must redial: {error}"));
    drive(&mut laptop, &mut phone, |_, _| {
        attachments_of(&phone_database, &meeting_id).len() == 2
    })
    .await;
    let received = attachments_of(&phone_database, &meeting_id);
    let arrived = received
        .iter()
        .find(|attachment| attachment.attachment_id() == later.attachment_id())
        .unwrap_or_else(|| panic!("the later attachment must arrive"));
    assert_eq!(
        clear_contents(&phone_database, arrived.file_path()),
        Some(b"later".to_vec())
    );
    assert_eq!(arrived.mime_type(), "text/plain");
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
        local_whisper_model: None,
        storage_device_id: None,
    });
    let mut phone = service("Phone", phone_adapter).with_local_capabilities(LocalCapabilities {
        cpu_cores: 4,
        gpu_vram_megabytes: 0,
        transcription_models: Vec::new(),
        local_whisper_model: None,
        storage_device_id: None,
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

/// Saves review clip `number` of `cluster_id` on `service`'s device.
fn record_clip(service: &Service, meeting_id: &MeetingId, cluster_id: Uuid, number: u8) {
    service
        .repository()
        .save_unassigned_clip(
            UnassignedVoiceClip::with_review_context(
                Uuid::from_u128(0x100 + u128::from(number)),
                meeting_id.clone(),
                u64::from(number) * 1_000,
                u64::from(number) * 1_000 + 900,
                vec![number; 64],
                vec![0.5, 0.25, -0.25],
                None,
            )
            .with_cluster_id(Some(cluster_id)),
        )
        .unwrap_or_else(|error| panic!("clip must be recorded: {error}"));
}

/// Counts the review clips of `meeting_id` stored on `service`'s device.
fn clip_count(service: &Service, meeting_id: &MeetingId) -> usize {
    service
        .repository()
        .get_unassigned_clips(meeting_id)
        .map(|clips| clips.len())
        .unwrap_or_default()
}

/// A meeting's unknown speakers can be named on another paired device (Issue #50): its clusters,
/// member embeddings, and review clips reach the phone once the pair key is agreed, even though
/// they are recorded before the meeting itself syncs, and a clip recorded later follows. Naming
/// the speaker on the phone then resolves the laptop's cluster and deletes its clips there.
#[tokio::test]
async fn unknown_speaker_clusters_reach_paired_devices() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9412"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let mut laptop = service("Laptop", laptop_adapter)
        .with_pair_agreement_secret(PairAgreementSecret::generate());
    let mut phone =
        service("Phone", phone_adapter).with_pair_agreement_secret(PairAgreementSecret::generate());
    let meeting_id = MeetingId::new("meeting-with-guests");
    let cluster_id = Uuid::from_u128(0xC1);
    MeetingCrdtService::new(laptop.repository())
        .create_titled_meeting(&meeting_id, &title("Guests"))
        .and_then(|()| {
            laptop.repository().save_unknown_speaker_cluster_chunk(
                &UnknownSpeakerCluster::new(
                    cluster_id,
                    meeting_id.clone(),
                    vec![0.5, 0.25, -0.25],
                    vec![String::from("chunk-a")],
                    1_000,
                    chrono::Utc::now(),
                    None,
                ),
                &UnknownSpeakerChunk::new("chunk-a", vec![0.5, 0.25, -0.25], 1_000),
            )
        })
        .unwrap_or_else(|error| panic!("cluster must be recorded: {error}"));
    record_clip(&laptop, &meeting_id, cluster_id, 1);
    record_clip(&laptop, &meeting_id, cluster_id, 2);

    pair(&mut laptop, &mut phone, phone_address).await;
    drive(&mut laptop, &mut phone, |_, phone| {
        clip_count(phone, &meeting_id) == 2
    })
    .await;

    let phone_clusters = phone
        .repository()
        .load_unknown_speaker_clusters(&meeting_id)
        .unwrap_or_else(|error| panic!("clusters must load: {error}"));
    assert_eq!(phone_clusters.len(), 1);
    assert_eq!(phone_clusters[0].cluster_id(), cluster_id);
    assert_eq!(
        phone
            .repository()
            .load_unknown_speaker_chunks(cluster_id)
            .unwrap_or_else(|error| panic!("chunks must load: {error}"))
            .len(),
        1
    );

    record_clip(&laptop, &meeting_id, cluster_id, 3);
    drive(&mut laptop, &mut phone, |_, phone| {
        clip_count(phone, &meeting_id) == 3
    })
    .await;
    assert_eq!(clip_count(&laptop, &meeting_id), 3);

    let assignment = assign_cluster_to_named_identity(
        phone.repository(),
        &InMemoryVoiceVectorDB::new(),
        cluster_id,
        "Giulia",
    )
    .unwrap_or_else(|error| panic!("the phone must name the speaker: {error}"));
    assert_eq!(clip_count(&phone, &meeting_id), 0);
    drive(&mut laptop, &mut phone, |laptop, _| {
        clip_count(laptop, &meeting_id) == 0
    })
    .await;
    assert_eq!(
        laptop
            .repository()
            .load_unknown_speaker_cluster(cluster_id)
            .unwrap_or_else(|error| panic!("cluster must load: {error}"))
            .and_then(|cluster| cluster.resolved_identity_id()),
        Some(assignment.identity_id())
    );
}

/// Opens another connection to a service's voice library, as the daemon's routes do.
fn voice_store(database_path: &PathBuf) -> SqliteVoiceVectorDB {
    SqliteVoiceVectorDB::open(
        database_path,
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
    )
    .unwrap_or_else(|error| panic!("voice store must open: {error}"))
}

/// The display names in a voice store.
fn voice_names(store: &SqliteVoiceVectorDB) -> Vec<String> {
    let mut names: Vec<String> = store
        .list_speaker_identities()
        .unwrap_or_else(|error| panic!("identities must list: {error}"))
        .iter()
        .map(|identity| identity.display_name().to_owned())
        .collect();
    names.sort();
    names
}

/// Enrolls `name` with one variant.
fn enroll(store: &SqliteVoiceVectorDB, identity_id: Uuid, name: &str, variant_id: Uuid) {
    store
        .save_speaker_identity(&SpeakerIdentity::new(identity_id, name))
        .and_then(|()| {
            store.store_embedding(VoiceEmbedding::new(identity_id, variant_id, vec![0.6, 0.8]))
        })
        .unwrap_or_else(|error| panic!("enrollment must succeed: {error}"));
}

/// A speaker named on one device is known on the others (Issue #56): libraries merge when devices
/// pair, a later rename and a later deletion follow, and a deleted voice does not come back.
#[tokio::test]
async fn the_voice_library_reaches_paired_devices() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9431"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (laptop, laptop_database) = service_with_database("Laptop", laptop_adapter);
    let (phone, phone_database) = service_with_database("Phone", phone_adapter);
    let mut laptop = laptop
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_voice_library(Box::new(voice_store(&laptop_database)));
    let mut phone = phone
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_voice_library(Box::new(voice_store(&phone_database)));
    let (on_laptop, on_phone) = (voice_store(&laptop_database), voice_store(&phone_database));
    let (alice, bruno) = (Uuid::from_u128(0xA1), Uuid::from_u128(0xB1));
    enroll(&on_laptop, alice, "Alice", Uuid::from_u128(0xA2));
    enroll(&on_phone, bruno, "Bruno", Uuid::from_u128(0xB2));

    pair(&mut laptop, &mut phone, phone_address).await;
    let both = vec!["Alice".to_owned(), "Bruno".to_owned()];
    drive(&mut laptop, &mut phone, |_, _| {
        voice_names(&on_laptop) == both && voice_names(&on_phone) == both
    })
    .await;
    assert_eq!(on_phone.voice_variant_count(alice).ok(), Some(1));
    assert_eq!(on_laptop.voice_variant_count(bruno).ok(), Some(1));

    on_phone
        .save_speaker_identity(&SpeakerIdentity::new(alice, "Alice Rossi"))
        .unwrap_or_else(|error| panic!("rename must succeed: {error}"));
    drive(&mut laptop, &mut phone, |_, _| {
        voice_names(&on_laptop).contains(&"Alice Rossi".to_owned())
    })
    .await;

    on_laptop
        .delete_identity(alice)
        .unwrap_or_else(|error| panic!("deletion must succeed: {error}"));
    drive(&mut laptop, &mut phone, |_, _| {
        voice_names(&on_phone) == vec!["Bruno".to_owned()]
    })
    .await;
    assert_eq!(on_phone.voice_variant_count(alice).ok(), Some(0));
    assert_eq!(voice_names(&on_laptop), vec!["Bruno".to_owned()]);
}

/// The names of the projects `database_path` lists, in list order.
fn project_names(database_path: &PathBuf) -> Vec<String> {
    pipeline_connection(database_path, "reader")
        .list_projects()
        .unwrap_or_default()
        .into_iter()
        .map(|summary| summary.project.name().as_str().to_owned())
        .collect()
}

/// A valid project name.
fn project_name(name: &str) -> ProjectName {
    ProjectName::parse(name).unwrap_or_else(|error| panic!("{error}"))
}

/// Projects made on either device reach the other (Issue #85), with a meeting moved into one; a
/// rename follows, and a deleted project goes from both and does not come back.
#[tokio::test]
async fn projects_reach_paired_devices() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9485"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (laptop, laptop_database) = service_with_database("Laptop", laptop_adapter);
    let (phone, phone_database) = service_with_database("Phone", phone_adapter);
    let mut laptop = laptop
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_project_library(Box::new(pipeline_connection(&laptop_database, "Laptop")));
    let mut phone = phone
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_project_library(Box::new(pipeline_connection(&phone_database, "Phone")));
    let (on_laptop, on_phone) = (
        pipeline_connection(&laptop_database, "Laptop"),
        pipeline_connection(&phone_database, "Phone"),
    );
    let hiring = on_laptop
        .create_project(&project_name("Hiring"))
        .unwrap_or_else(|error| panic!("{error}"));
    on_phone
        .create_project(&project_name("Budget"))
        .unwrap_or_else(|error| panic!("{error}"));
    let interview = MeetingId::new("meeting-interview");
    MeetingCrdtService::new(pipeline_connection(&laptop_database, "Laptop"))
        .create_meeting(&interview)
        .unwrap_or_else(|error| panic!("{error}"));
    on_laptop
        .move_meeting_to_project(&interview, Some(hiring.project_id()))
        .unwrap_or_else(|error| panic!("{error}"));

    pair(&mut laptop, &mut phone, phone_address).await;
    let both = vec!["Budget".to_owned(), "Hiring".to_owned()];
    drive(&mut laptop, &mut phone, |_, _| {
        project_names(&laptop_database) == both
            && project_names(&phone_database) == both
            && on_phone.meeting_projects().unwrap_or_default()
                == vec![(interview.clone(), hiring.project_id())]
    })
    .await;

    on_phone
        .rename_project(hiring.project_id(), &project_name("Hiring 2027"))
        .unwrap_or_else(|error| panic!("{error}"));
    drive(&mut laptop, &mut phone, |_, _| {
        project_names(&laptop_database).contains(&"Hiring 2027".to_owned())
    })
    .await;

    on_laptop
        .delete_project(hiring.project_id())
        .unwrap_or_else(|error| panic!("{error}"));
    drive(&mut laptop, &mut phone, |_, _| {
        project_names(&phone_database) == vec!["Budget".to_owned()]
    })
    .await;
    assert_eq!(project_names(&laptop_database), vec!["Budget".to_owned()]);
    assert!(on_phone.meeting_projects().unwrap_or_default().is_empty());
    assert!(
        on_phone
            .load_meeting_metadata(&interview)
            .ok()
            .flatten()
            .is_some()
    );
}

/// A device that lost the other's agreement key gets it again (Issue #85). On the test VMs the
/// device that confirmed the pairing first sent its key before the other admitted it, so the key
/// was dropped and every project list after it too. Here the laptop's pinned key is wiped after
/// pairing: the project list the phone then seals must still reach it, because the laptop asks
/// for the key again and the phone sends the list again once both hold it.
#[tokio::test]
async fn a_device_that_lost_the_pair_key_asks_for_it_and_gets_the_projects() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9486"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (laptop, laptop_database) = service_with_database("Laptop", laptop_adapter);
    let (phone, phone_database) = service_with_database("Phone", phone_adapter);
    let mut laptop = laptop
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_project_library(Box::new(pipeline_connection(&laptop_database, "Laptop")));
    let mut phone = phone
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_project_library(Box::new(pipeline_connection(&phone_database, "Phone")));
    let phone_id = phone.transport().local_peer_id();
    let laptop_id = laptop.transport().local_peer_id();
    let laptop_store = trust_store_connection(&laptop_database);
    let phone_store = trust_store_connection(&phone_database);
    pair(&mut laptop, &mut phone, phone_address).await;
    drive(&mut laptop, &mut phone, |_, _| {
        matches!(laptop_store.pair_key(&phone_id), Ok(Some(_)))
            && matches!(phone_store.pair_key(&laptop_id), Ok(Some(_)))
    })
    .await;

    // The laptop forgets the phone's key, as if the phone's key message had been dropped.
    rusqlite::Connection::open(&laptop_database)
        .and_then(|connection| {
            connection.execute(
                "UPDATE p2p_trusted_peers SET sealed_pair_key = NULL WHERE peer_id = ?1",
                [phone_id.as_str()],
            )
        })
        .unwrap_or_else(|error| panic!("the pinned key must be wiped: {error}"));
    pipeline_connection(&phone_database, "Phone")
        .create_project(&project_name("Hiring"))
        .unwrap_or_else(|error| panic!("{error}"));

    drive(&mut laptop, &mut phone, |_, _| {
        project_names(&laptop_database) == vec!["Hiring".to_owned()]
    })
    .await;
    let on_laptop = laptop_store
        .pair_key(&phone_id)
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("the laptop must hold the pair key again"));
    let on_phone = phone_store
        .pair_key(&laptop_id)
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("the phone must keep the pair key"));
    assert!(on_laptop.matches(&on_phone));
}

/// A meeting deleted on one device goes from the other too, whichever way it is reported, and
/// neither device's sync brings it back.
#[tokio::test]
async fn a_deleted_meeting_goes_from_paired_devices_and_stays_gone() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9441"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (mut laptop, _) = service_with_database("Laptop", laptop_adapter);
    let (mut phone, _) = service_with_database("Phone", phone_adapter);
    let (doomed, kept) = (
        MeetingId::new("meeting-doomed"),
        MeetingId::new("meeting-kept"),
    );
    for meeting_id in [&doomed, &kept] {
        MeetingCrdtService::new(laptop.repository())
            .create_meeting(meeting_id)
            .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
    }
    let lists = |service: &Service, meeting_id: &MeetingId| {
        service
            .repository()
            .list_meeting_ids()
            .unwrap_or_else(|error| panic!("meetings must list: {error}"))
            .contains(meeting_id)
    };

    pair(&mut laptop, &mut phone, phone_address).await;
    drive(&mut laptop, &mut phone, |_, phone| {
        lists(phone, &doomed) && lists(phone, &kept)
    })
    .await;

    laptop
        .repository()
        .delete_meeting(&doomed)
        .unwrap_or_else(|error| panic!("deletion must succeed: {error}"));
    drive(&mut laptop, &mut phone, |_, phone| !lists(phone, &doomed)).await;

    // The kept meeting still syncs, and a further round restores nothing on either device.
    MeetingCrdtService::new(phone.repository())
        .create_meeting(&MeetingId::new("meeting-later"))
        .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
    drive(&mut laptop, &mut phone, |laptop, _| {
        lists(laptop, &MeetingId::new("meeting-later"))
    })
    .await;
    assert!(!lists(&laptop, &doomed));
    assert!(!lists(&phone, &doomed));
    assert!(lists(&phone, &kept));
}

/// One part an inbox stored: the sender, the meeting, the part's index, and its samples.
type StoredPart = (DeviceId, MeetingId, u32, Vec<u8>);

/// Records the speech parts it is given, or refuses them all.
#[derive(Clone, Default)]
struct FakeInbox {
    refuse: bool,
    stored: Arc<Mutex<Vec<StoredPart>>>,
}

impl SpeechInbox for FakeInbox {
    fn receive_part(
        &mut self,
        from: &DeviceId,
        part: SpeechPart,
    ) -> Result<(), SpeechTransferError> {
        if self.refuse {
            return Err(SpeechTransferError::Refused);
        }
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((
                from.clone(),
                part.meeting_id,
                part.part_index,
                part.chunks
                    .iter()
                    .flat_map(|chunk| chunk.speech.iter().copied())
                    .collect(),
            ));
        Ok(())
    }
}

/// Pairs a phone and a laptop whose speech inbox is `inbox`, with agreed pair keys (#88).
async fn paired_with_inbox(memory_address: &str, inbox: FakeInbox) -> (Service, Service) {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = memory_address
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let mut laptop = service("Laptop", laptop_adapter)
        .with_local_capabilities(stored_as("device-laptop"))
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_speech_inbox(Box::new(inbox));
    let (phone, phone_database) = service_with_database("Phone", phone_adapter);
    let mut phone = phone
        .with_local_capabilities(stored_as("device-phone"))
        .with_pair_agreement_secret(PairAgreementSecret::generate());
    let phone_store = trust_store_connection(&phone_database);
    pair(&mut laptop, &mut phone, phone_address).await;
    let laptop_id = laptop.transport().local_peer_id();
    drive(&mut laptop, &mut phone, |laptop, phone| {
        phone_store.pair_key(&laptop_id).ok().flatten().is_some()
            && !laptop.status().peer_manifests.is_empty()
            && !phone.status().peer_manifests.is_empty()
    })
    .await;
    (laptop, phone)
}

/// Capabilities that name the ID a device writes on its meetings.
fn stored_as(storage_device_id: &str) -> LocalCapabilities {
    LocalCapabilities {
        storage_device_id: Some(DeviceId::new(storage_device_id)),
        ..LocalCapabilities::default()
    }
}

/// Sends one part of the phone's speech to the laptop and returns where the outcome arrives.
fn send_speech(phone: &mut Service) -> std::sync::mpsc::Receiver<Result<(), SpeechTransferError>> {
    let (reply, outcome) = std::sync::mpsc::channel();
    phone.request_speech_transfer(SpeechTransferJob {
        device: DeviceId::new("device-laptop"),
        part: SpeechPart {
            meeting_id: MeetingId::new("meeting-phone"),
            part_index: 0,
            part_count: 1,
            chunks: vec![SpeechChunk {
                start_ms: 0,
                end_ms: 1,
                language: None,
                speech: zeroize::Zeroizing::new(vec![4, 5, 6]),
            }],
        },
        reply,
    });
    outcome
}

/// A part of the phone's speech reaches the laptop's inbox, and the phone hears it was stored.
#[tokio::test]
async fn speech_reaches_the_combining_device_and_is_confirmed() {
    let inbox = FakeInbox::default();
    let stored = Arc::clone(&inbox.stored);
    let (mut laptop, mut phone) = paired_with_inbox("/memory/9481", inbox).await;

    let outcome = send_speech(&mut phone);
    let mut result = None;
    drive(&mut laptop, &mut phone, |_, _| {
        result = outcome.try_recv().ok();
        result.is_some()
    })
    .await;

    assert_eq!(result, Some(Ok(())));
    let stored = stored.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].0,
        DeviceId::new("device-phone"),
        "named as on its meetings"
    );
    assert_eq!(stored[0].1, MeetingId::new("meeting-phone"));
    assert_eq!(stored[0].3, vec![4, 5, 6]);
}

/// A device that does not take the speech says so, and the sender keeps its copy.
#[tokio::test]
async fn refused_speech_is_reported_to_the_sender() {
    let inbox = FakeInbox {
        refuse: true,
        ..FakeInbox::default()
    };
    let (mut laptop, mut phone) = paired_with_inbox("/memory/9482", inbox).await;

    let outcome = send_speech(&mut phone);
    let mut result = None;
    drive(&mut laptop, &mut phone, |_, _| {
        result = outcome.try_recv().ok();
        result.is_some()
    })
    .await;

    assert_eq!(result, Some(Err(SpeechTransferError::Refused)));
}

/// Speech for a device that is not connected fails at once.
#[tokio::test]
async fn speech_for_an_absent_device_fails_at_once() {
    let adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone = service("Phone", adapter);
    let (reply, outcome) = std::sync::mpsc::channel();

    phone.request_speech_transfer(SpeechTransferJob {
        device: DeviceId::new("device-away"),
        part: SpeechPart {
            meeting_id: MeetingId::new("meeting-phone"),
            part_index: 0,
            part_count: 1,
            chunks: Vec::new(),
        },
        reply,
    });

    assert_eq!(
        outcome.try_recv(),
        Ok(Err(SpeechTransferError::Unreachable))
    );
}

/// A paired device's storage ID is remembered, so its meetings keep its name while it is away.
#[tokio::test]
async fn a_paired_device_s_storage_id_is_remembered_with_its_pairing() {
    let (laptop, phone) = paired_with_inbox("/memory/9487", FakeInbox::default()).await;

    let remembered = |service: &Service| {
        service
            .status()
            .trusted_devices
            .iter()
            .filter_map(|peer| peer.storage_device_id().cloned())
            .collect::<Vec<_>>()
    };
    assert_eq!(remembered(&laptop), vec![DeviceId::new("device-phone")]);
    assert_eq!(remembered(&phone), vec![DeviceId::new("device-laptop")]);
}

/// A chat host that answers every question with the question's own text, or declines.
struct FakeChatHost {
    decline: Option<String>,
    finished: Vec<crate::application::p2p_service::FinishedPairedChat>,
}

impl crate::application::p2p_service::PairedChatHost for FakeChatHost {
    fn submit(
        &mut self,
        peer: DeviceId,
        request: crate::application::paired_chat_exchange::PairedChatRequest,
    ) -> Result<(), String> {
        if let Some(code) = &self.decline {
            return Err(code.clone());
        }
        self.finished
            .push(crate::application::p2p_service::FinishedPairedChat {
                peer,
                job_id: request.job_id,
                outcome: crate::application::paired_chat_exchange::PairedChatOutcome::Answered(
                    crate::application::paired_chat_exchange::PairedChatAnswer {
                        text: format!(
                            "answer to {} after {} turns",
                            request.question,
                            request.history.len()
                        ),
                        included: vec!["transcript".to_owned()],
                        omitted: Vec::new(),
                        sources: Vec::new(),
                        connection_label: "Mock on Laptop".to_owned(),
                        truncated: false,
                        images_sent: 0,
                    },
                ),
            });
        Ok(())
    }

    fn poll_finished(&mut self) -> Option<crate::application::p2p_service::FinishedPairedChat> {
        self.finished.pop()
    }
}

/// Asks the phone's paired computers a chat question and waits for the result.
async fn ask_chat(
    laptop: &mut Service,
    phone: &mut Service,
) -> Option<crate::application::paired_chat_relay::PairedChatResult> {
    let (reply, answer) = std::sync::mpsc::channel();
    phone.request_paired_chat(crate::application::paired_chat_relay::PairedChatJob {
        scope: crate::domain::chat::ChatScope::All,
        question: "What is open?".to_owned(),
        history: vec![crate::application::paired_chat_exchange::PairedChatTurn {
            role: crate::domain::chat::ChatRole::User,
            text: "Hello".to_owned(),
        }],
        reply,
    });
    let mut result = None;
    drive(laptop, phone, |_, _| {
        result = answer.try_recv().ok();
        result.is_some()
    })
    .await;
    result
}

/// A computer that answers for its devices answers the phone's chat question.
#[tokio::test]
async fn a_paired_computer_answers_a_chat_question() {
    let (laptop, mut phone) = paired_with_host("/memory/9611", FakeHost::default()).await;
    let mut laptop = laptop.with_paired_chat_host(Box::new(FakeChatHost {
        decline: None,
        finished: Vec::new(),
    }));

    let result = ask_chat(&mut laptop, &mut phone).await;

    let answer = result.and_then(Result::ok).map(|answer| answer.text);
    assert_eq!(
        answer.as_deref(),
        Some("answer to What is open? after 1 turns")
    );
}

/// A computer that does not answer for its devices declines, and the phone hears why at once.
#[tokio::test]
async fn a_computer_without_chat_declines_the_question() {
    let (mut laptop, mut phone) = paired_with_host("/memory/9612", FakeHost::default()).await;
    let result = ask_chat(&mut laptop, &mut phone).await;
    assert_eq!(result, Some(Err("no_paired_computer".to_owned())));

    let mut laptop = laptop.with_paired_chat_host(Box::new(FakeChatHost {
        decline: Some("llm_unreachable".to_owned()),
        finished: Vec::new(),
    }));
    let result = ask_chat(&mut laptop, &mut phone).await;
    assert_eq!(result, Some(Err("llm_unreachable".to_owned())));
}

/// Prompts kept in memory, as a host keeps them with its settings (#145).
#[derive(Clone, Default)]
struct MemoryPrompts(Arc<Mutex<crate::domain::shared_prompts::SharedPrompts>>);

impl MemoryPrompts {
    fn get(&self) -> crate::domain::shared_prompts::SharedPrompts {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set(&self, prompts: crate::domain::shared_prompts::SharedPrompts) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = prompts;
    }
}

impl crate::ports::shared_prompts::SharedPromptsSync for MemoryPrompts {
    fn shared_prompts(
        &self,
    ) -> Result<crate::domain::shared_prompts::SharedPrompts, crate::domain::errors::StorageError>
    {
        Ok(self.get())
    }

    fn merge_shared_prompts(
        &self,
        incoming: &crate::domain::shared_prompts::SharedPrompts,
    ) -> Result<bool, crate::domain::errors::StorageError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .merge(incoming))
    }
}

/// A prompt version with text.
fn prompt_version(
    text: Option<&str>,
    changed_at_ms: u64,
) -> crate::domain::shared_prompts::PromptVersion {
    crate::domain::shared_prompts::PromptVersion {
        text: text.map(str::to_owned),
        changed_at_ms,
    }
}

/// The owner's summary prompt edited on the laptop and chat prompt edited on the phone reach
/// both devices (#145); a later reset to the built-in chat prompt reaches the phone too.
#[tokio::test]
async fn prompts_reach_paired_devices() {
    let laptop_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let mut phone_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
    let phone_address: Multiaddr = "/memory/9614"
        .parse()
        .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
    phone_adapter
        .listen_on_memory(phone_address.clone())
        .unwrap_or_else(|error| panic!("phone must listen: {error}"));
    let (on_laptop, on_phone) = (MemoryPrompts::default(), MemoryPrompts::default());
    on_laptop.set(crate::domain::shared_prompts::SharedPrompts {
        summary: prompt_version(Some("Only decisions for {{title}}."), 2_000),
        chat: prompt_version(Some("Old chat about {{scope}}."), 1_000),
    });
    on_phone.set(crate::domain::shared_prompts::SharedPrompts {
        summary: prompt_version(None, 0),
        chat: prompt_version(Some("Short answers about {{scope}}."), 3_000),
    });
    let (laptop, _laptop_database) = service_with_database("Laptop", laptop_adapter);
    let (phone, _phone_database) = service_with_database("Phone", phone_adapter);
    let mut laptop = laptop
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_shared_prompts(Box::new(on_laptop.clone()));
    let mut phone = phone
        .with_pair_agreement_secret(PairAgreementSecret::generate())
        .with_shared_prompts(Box::new(on_phone.clone()));

    pair(&mut laptop, &mut phone, phone_address).await;
    let expected = crate::domain::shared_prompts::SharedPrompts {
        summary: prompt_version(Some("Only decisions for {{title}}."), 2_000),
        chat: prompt_version(Some("Short answers about {{scope}}."), 3_000),
    };
    drive(&mut laptop, &mut phone, |_, _| {
        on_laptop.get() == expected && on_phone.get() == expected
    })
    .await;

    let mut reset = on_laptop.get();
    reset.chat = prompt_version(None, 4_000);
    on_laptop.set(reset.clone());
    drive(&mut laptop, &mut phone, |_, _| on_phone.get() == reset).await;
}
