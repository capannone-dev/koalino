//! Specifies the `/api/p2p/*` routes the Devices screens use (Issue #43).
//!
//! A fake `P2pControl` records commands and returns scripted outcomes, so these tests cover the
//! HTTP contract (paths, JSON shapes, status codes, input validation) without libp2p.

use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        p2p_service::{
            P2pCommand, P2pCommandOutcome, P2pControl, P2pControlError, P2pSnapshot, P2pStatus,
        },
        pairing_manager::{IncomingPairingRequest, OutgoingPairingStatus, PairingUpdate},
    },
    domain::{
        models::{DeviceId, DeviceManifest, MeetingId, TranscriptionModel},
        peer_trust::{DeviceName, PairingCode, TrustedPeer},
    },
    server::api::{ServerAppState, build_router},
};

/// A well-formed peer ID used by the fixtures.
const PHONE: &str = "12D3KooWPhone";

/// Scripted outcome for every command.
type Script = Box<dyn Fn(&P2pCommand) -> Result<P2pCommandOutcome, P2pControlError> + Send + Sync>;

/// Records commands and answers them from a script.
struct FakeControl {
    commands: Mutex<Vec<P2pCommand>>,
    /// Meetings reported through `notify_local_change`.
    local_changes: Mutex<Vec<MeetingId>>,
    script: Script,
}

#[async_trait::async_trait]
impl P2pControl for FakeControl {
    fn snapshot(&self) -> P2pSnapshot {
        snapshot()
    }

    async fn command(&self, command: P2pCommand) -> Result<P2pCommandOutcome, P2pControlError> {
        let outcome = (self.script)(&command);
        self.commands
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(command);
        outcome
    }

    fn notify_local_change(&self, meeting_id: &MeetingId) {
        self.local_changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(meeting_id.clone());
    }
}

/// Parses a fixture device name.
fn name(text: &str) -> DeviceName {
    DeviceName::parse(text).unwrap_or_else(|error| panic!("valid name: {error}"))
}

/// A snapshot with one entry in every list.
fn snapshot() -> P2pSnapshot {
    let laptop = DeviceId::new("12D3KooWLaptop");
    let phone = DeviceId::new(PHONE);
    let tablet = DeviceId::new("12D3KooWTablet");
    P2pSnapshot {
        status: P2pStatus {
            local_peer_id: laptop.clone(),
            device_name: name("Laptop"),
            trusted_devices: vec![TrustedPeer::new(phone.clone(), name("Phone"), 1_700)],
            nearby_devices: vec![tablet.clone()],
            incoming_requests: vec![IncomingPairingRequest {
                peer_id: tablet.clone(),
                device_name: name("Tablet"),
                code: PairingCode::for_peers(&laptop, &tablet),
            }],
            outgoing_pairings: vec![OutgoingPairingStatus {
                peer_id: phone.clone(),
                code: PairingCode::for_peers(&laptop, &phone),
                locally_confirmed: false,
            }],
            recent_updates: vec![PairingUpdate::Rejected { peer_id: tablet }],
            local_manifest: DeviceManifest::new(
                laptop.clone(),
                8,
                4_096,
                vec![
                    TranscriptionModel::Whisper,
                    TranscriptionModel::ThirdParty("openai".to_owned()),
                ],
            ),
            peer_manifests: vec![DeviceManifest::new(phone.clone(), 4, 0, Vec::new())],
        },
        listen_addresses: vec!["/ip4/127.0.0.1/tcp/4001".to_owned()],
    }
}

/// Builds a router whose P2P control answers with `script`, and returns the control too.
fn router_with(script: Script) -> (axum::Router, Arc<FakeControl>) {
    let control = Arc::new(FakeControl {
        commands: Mutex::new(Vec::new()),
        local_changes: Mutex::new(Vec::new()),
        script,
    });
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let state = ServerAppState::new(repository, InMemoryVoiceVectorDB::new())
        .with_p2p_control(Arc::clone(&control) as Arc<dyn P2pControl>);
    (build_router(state), control)
}

/// Builds a router whose commands all succeed with `Done`.
fn router() -> (axum::Router, Arc<FakeControl>) {
    router_with(Box::new(|_| Ok(P2pCommandOutcome::Done)))
}

/// Sends one request and returns its status and body as JSON (`Null` when empty).
async fn send(
    router: axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = router
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |text| Body::from(text.to_owned())))
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap_or_else(|error| panic!("body must read: {error}"));
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("body must be JSON: {error}"))
    };
    (status, json)
}

/// The commands the fake control received.
fn received(control: &FakeControl) -> Vec<P2pCommand> {
    control
        .commands
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Without a P2P task, status reports it disabled and commands are unavailable.
#[tokio::test]
async fn without_p2p_status_is_disabled_and_commands_are_unavailable() {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let router = build_router(ServerAppState::new(
        repository,
        InMemoryVoiceVectorDB::new(),
    ));

    let (status_code, status) = send(router.clone(), "GET", "/api/p2p/status", None).await;
    let (pairable_code, _) = send(router, "POST", "/api/p2p/pairable", None).await;

    assert_eq!(status_code, StatusCode::OK);
    assert_eq!(status, serde_json::json!({ "enabled": false }));
    assert_eq!(pairable_code, StatusCode::SERVICE_UNAVAILABLE);
}

/// Status exposes every list of the snapshot with stable field names.
#[tokio::test]
async fn status_lists_devices_pairings_and_updates() {
    let (router, _) = router();

    let (status_code, status) = send(router, "GET", "/api/p2p/status", None).await;

    let laptop = DeviceId::new("12D3KooWLaptop");
    let tablet_code = PairingCode::for_peers(&laptop, &DeviceId::new("12D3KooWTablet"));
    let phone_code = PairingCode::for_peers(&laptop, &DeviceId::new(PHONE));
    assert_eq!(status_code, StatusCode::OK);
    assert_eq!(
        status,
        serde_json::json!({
            "enabled": true,
            "local_peer_id": "12D3KooWLaptop",
            "device_name": "Laptop",
            "listen_addresses": ["/ip4/127.0.0.1/tcp/4001"],
            "capabilities": {
                "cpu_cores": 8,
                "gpu_vram_megabytes": 4096,
                "transcription_models": ["whisper", "openai"],
                "supports_vad": cfg!(feature = "vad")
            },
            "trusted_devices": [
                {
                    "peer_id": PHONE,
                    "device_name": "Phone",
                    "paired_at_ms": 1700,
                    "capabilities": {
                        "cpu_cores": 4,
                        "gpu_vram_megabytes": 0,
                        "transcription_models": [],
                        "supports_vad": cfg!(feature = "vad")
                    }
                }
            ],
            "nearby_devices": [{ "peer_id": "12D3KooWTablet" }],
            "incoming_requests": [
                { "peer_id": "12D3KooWTablet", "device_name": "Tablet", "code": tablet_code.as_str() }
            ],
            "outgoing_pairings": [
                { "peer_id": PHONE, "code": phone_code.as_str(), "locally_confirmed": false }
            ],
            "recent_updates": [{ "kind": "rejected", "peer_id": "12D3KooWTablet" }]
        })
    );
}

/// Starting a pairing returns the code; each pairing step sends its command.
#[tokio::test]
async fn pairing_steps_send_their_commands() {
    let phone = DeviceId::new(PHONE);
    let code = PairingCode::for_peers(&DeviceId::new("12D3KooWLaptop"), &phone);
    let scripted_code = code.clone();
    let (router, control) = router_with(Box::new(move |command| {
        Ok(match command {
            P2pCommand::StartPairing(_) => P2pCommandOutcome::PairingStarted(scripted_code.clone()),
            P2pCommand::ConfirmCode(peer_id) => {
                P2pCommandOutcome::Pairing(PairingUpdate::AwaitingRemote {
                    peer_id: peer_id.clone(),
                })
            }
            P2pCommand::AcceptIncoming(peer_id) => {
                P2pCommandOutcome::Pairing(PairingUpdate::Paired {
                    peer_id: peer_id.clone(),
                    device_name: DeviceName::parse("Phone")
                        .unwrap_or_else(|error| panic!("valid name: {error}")),
                })
            }
            _ => P2pCommandOutcome::Done,
        })
    }));

    let pairable = send(router.clone(), "POST", "/api/p2p/pairable", None).await;
    let started = send(
        router.clone(),
        "POST",
        "/api/p2p/pairings",
        Some(&format!(r#"{{"peer_id":"{PHONE}"}}"#)),
    )
    .await;
    let confirmed = send(
        router.clone(),
        "POST",
        &format!("/api/p2p/pairings/{PHONE}/confirm"),
        None,
    )
    .await;
    let cancelled = send(
        router.clone(),
        "POST",
        &format!("/api/p2p/pairings/{PHONE}/cancel"),
        None,
    )
    .await;
    let accepted = send(
        router.clone(),
        "POST",
        &format!("/api/p2p/requests/{PHONE}/accept"),
        None,
    )
    .await;
    let rejected = send(
        router,
        "POST",
        &format!("/api/p2p/requests/{PHONE}/reject"),
        None,
    )
    .await;

    assert_eq!(pairable.0, StatusCode::NO_CONTENT);
    assert_eq!(
        started,
        (StatusCode::OK, serde_json::json!({ "code": code.as_str() }))
    );
    assert_eq!(
        confirmed,
        (
            StatusCode::OK,
            serde_json::json!({ "kind": "awaiting_remote", "peer_id": PHONE })
        )
    );
    assert_eq!(cancelled.0, StatusCode::NO_CONTENT);
    assert_eq!(
        accepted,
        (
            StatusCode::OK,
            serde_json::json!({ "kind": "paired", "peer_id": PHONE, "device_name": "Phone" })
        )
    );
    assert_eq!(rejected.0, StatusCode::NO_CONTENT);
    assert_eq!(
        received(&control),
        vec![
            P2pCommand::MakePairable,
            P2pCommand::StartPairing(phone.clone()),
            P2pCommand::ConfirmCode(phone.clone()),
            P2pCommand::CancelPairing(phone.clone()),
            P2pCommand::AcceptIncoming(phone.clone()),
            P2pCommand::RejectIncoming(phone),
        ]
    );
}

/// Revoking a paired device answers 204; an unknown one answers 404.
#[tokio::test]
async fn revoking_reports_whether_the_device_was_paired() {
    let (router, control) = router_with(Box::new(|command| {
        Ok(P2pCommandOutcome::Revoked(matches!(
            command,
            P2pCommand::Revoke(peer_id) if peer_id.as_str() == PHONE
        )))
    }));

    let revoked = send(
        router.clone(),
        "DELETE",
        &format!("/api/p2p/devices/{PHONE}"),
        None,
    )
    .await;
    let unknown = send(router, "DELETE", "/api/p2p/devices/12D3KooWOther", None).await;

    assert_eq!(revoked.0, StatusCode::NO_CONTENT);
    assert_eq!(unknown.0, StatusCode::NOT_FOUND);
    assert_eq!(received(&control).len(), 2);
}

/// A refused command answers 409 with its reason, a stopped task 503, and a malformed peer ID
/// 400 without reaching the task.
#[tokio::test]
async fn failures_map_to_status_codes_and_bad_peer_ids_never_reach_the_task() {
    let (router, control) = router_with(Box::new(|command| match command {
        P2pCommand::ConfirmCode(_) => Err(P2pControlError::Refused(
            "no pairing with this device is in progress".to_owned(),
        )),
        _ => Err(P2pControlError::Stopped),
    }));

    let refused = send(
        router.clone(),
        "POST",
        &format!("/api/p2p/pairings/{PHONE}/confirm"),
        None,
    )
    .await;
    let stopped = send(router.clone(), "POST", "/api/p2p/pairable", None).await;
    let malformed_path = send(
        router.clone(),
        "POST",
        "/api/p2p/requests/not%20a%20peer/accept",
        None,
    )
    .await;
    let too_long = "a".repeat(129);
    let malformed_body = send(
        router,
        "POST",
        "/api/p2p/pairings",
        Some(&format!(r#"{{"peer_id":"{too_long}"}}"#)),
    )
    .await;

    assert_eq!(
        refused,
        (
            StatusCode::CONFLICT,
            serde_json::json!({ "error": "no pairing with this device is in progress" })
        )
    );
    assert_eq!(stopped.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(malformed_path.0, StatusCode::BAD_REQUEST);
    assert_eq!(malformed_body.0, StatusCode::BAD_REQUEST);
    assert_eq!(received(&control).len(), 2);
}

/// The dashboard origin may send DELETE, which revocation uses.
#[tokio::test]
async fn cors_preflight_allows_delete_from_the_web_ui() {
    let (router, _) = router();

    let response = router
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri(format!("/api/p2p/devices/{PHONE}"))
                .header("origin", "http://localhost:3000")
                .header("access-control-request-method", "DELETE")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

    let allowed_methods = response
        .headers()
        .get("access-control-allow-methods")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(allowed_methods.contains("DELETE"), "{allowed_methods:?}");
}

/// Creating and renaming a meeting over HTTP report it to the P2P task, so connected devices get
/// the change at once; a failed rename reports nothing.
#[tokio::test]
async fn meeting_writes_are_reported_to_the_p2p_task() {
    let (router, control) = router();

    let (created_code, created) = send(
        router.clone(),
        "POST",
        "/api/meetings",
        Some(r#"{"title":"Standup"}"#),
    )
    .await;
    let meeting_id = created["meeting_id"]
        .as_str()
        .unwrap_or_else(|| panic!("the new meeting must have an ID: {created}"))
        .to_owned();
    let renamed = send(
        router.clone(),
        "PATCH",
        &format!("/api/meetings/{meeting_id}"),
        Some(r#"{"title":"Weekly standup"}"#),
    )
    .await;
    let invalid = send(
        router,
        "PATCH",
        &format!("/api/meetings/{meeting_id}"),
        Some(r#"{"title":"   "}"#),
    )
    .await;

    assert_eq!(created_code, StatusCode::CREATED);
    assert_eq!(renamed.0, StatusCode::OK);
    assert_eq!(invalid.0, StatusCode::BAD_REQUEST);
    let local_changes = control
        .local_changes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        local_changes,
        vec![
            MeetingId::new(meeting_id.clone()),
            MeetingId::new(meeting_id)
        ]
    );
}
