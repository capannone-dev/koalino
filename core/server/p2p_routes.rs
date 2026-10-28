//! Serves the `/api/p2p/*` routes the Devices screens use to pair and manage devices (Issue #43).
//!
//! | Route | Command | Success |
//! | --- | --- | --- |
//! | `GET /api/p2p/status` | none (reads the snapshot) | 200, `{"enabled": false}` without P2P |
//! | `POST /api/p2p/pairable` | `MakePairable` | 204 |
//! | `POST /api/p2p/pairings` `{"peer_id"}` | `StartPairing` | 200 `{"code"}` |
//! | `POST /api/p2p/pairings/{peer_id}/confirm` | `ConfirmCode` | 200, the pairing update |
//! | `POST /api/p2p/pairings/{peer_id}/cancel` | `CancelPairing` | 204 |
//! | `POST /api/p2p/requests/{peer_id}/accept` | `AcceptIncoming` | 200, the pairing update |
//! | `POST /api/p2p/requests/{peer_id}/reject` | `RejectIncoming` | 204 |
//! | `DELETE /api/p2p/devices/{peer_id}` | `Revoke` | 204, or 404 when not paired |
//!
//! Failures:
//!
//! - 400: a peer ID that is not 1–128 ASCII letters and digits; the request never reaches the task.
//! - 409 `{"error"}`: the task refused the command, for example for a pairing not in progress.
//! - 503: P2P is disabled or its task stopped.
//!
//! The routes sit behind the same request guard as the rest of the API, so a web page on another
//! origin cannot pair a device through the owner's browser.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};

use super::api::ServerAppState;
use crate::{
    application::{
        p2p_service::{P2pCommand, P2pCommandOutcome, P2pControl, P2pControlError},
        pairing_manager::PairingUpdate,
    },
    domain::models::DeviceId,
};

/// Longest accepted peer ID; libp2p Ed25519 peer IDs are 52 characters.
const MAX_PEER_ID_CHARACTERS: usize = 128;

/// Registers the P2P routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new()
        .route("/api/p2p/status", get(status))
        .route("/api/p2p/pairable", post(make_pairable))
        .route("/api/p2p/pairings", post(start_pairing))
        .route("/api/p2p/pairings/{peer_id}/confirm", post(confirm_code))
        .route("/api/p2p/pairings/{peer_id}/cancel", post(cancel_pairing))
        .route("/api/p2p/requests/{peer_id}/accept", post(accept_request))
        .route("/api/p2p/requests/{peer_id}/reject", post(reject_request))
        .route("/api/p2p/devices/{peer_id}", delete(revoke_device))
}

/// Why a P2P route failed, mapped to its status code.
enum P2pRouteFailure {
    /// The peer ID is malformed (400).
    BadPeerId,
    /// P2P is disabled or stopped (503).
    Unavailable,
    /// The task refused the command (409 with the reason).
    Refused(String),
    /// The device to revoke was not paired (404).
    NotPaired,
    /// The task answered with an outcome that does not fit the command (500).
    UnexpectedOutcome,
}

impl IntoResponse for P2pRouteFailure {
    fn into_response(self) -> Response {
        match self {
            Self::BadPeerId => StatusCode::BAD_REQUEST.into_response(),
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            Self::Refused(reason) => {
                (StatusCode::CONFLICT, Json(ErrorResponse { error: reason })).into_response()
            }
            Self::NotPaired => StatusCode::NOT_FOUND.into_response(),
            Self::UnexpectedOutcome => {
                tracing::error!("the P2P task answered a command with an unexpected outcome");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

/// Body of a 409 answer.
#[derive(Serialize)]
struct ErrorResponse {
    /// Why the task refused the command; it never contains user content.
    error: String,
}

/// Body of `POST /api/p2p/pairings`.
#[derive(Deserialize)]
struct StartPairingRequest {
    /// A nearby device from the status's `nearby_devices`.
    peer_id: String,
}

/// Answer of `POST /api/p2p/pairings`.
#[derive(Serialize)]
struct PairingCodeResponse {
    /// The six-digit code both devices show.
    code: String,
}

/// Answer of `GET /api/p2p/status`.
#[derive(Serialize)]
struct StatusResponse {
    /// Whether the P2P task runs; every other field is absent when it does not.
    enabled: bool,
    #[serde(flatten)]
    details: Option<StatusDetails>,
}

/// The running task's state.
#[derive(Serialize)]
struct StatusDetails {
    local_peer_id: String,
    device_name: String,
    listen_addresses: Vec<String>,
    trusted_devices: Vec<TrustedDeviceResponse>,
    nearby_devices: Vec<NearbyDeviceResponse>,
    incoming_requests: Vec<IncomingRequestResponse>,
    outgoing_pairings: Vec<OutgoingPairingResponse>,
    recent_updates: Vec<PairingUpdateResponse>,
}

/// A paired device.
#[derive(Serialize)]
struct TrustedDeviceResponse {
    peer_id: String,
    device_name: String,
    paired_at_ms: u64,
}

/// An unpaired device on the local network.
#[derive(Serialize)]
struct NearbyDeviceResponse {
    peer_id: String,
}

/// A request awaiting the owner's answer.
#[derive(Serialize)]
struct IncomingRequestResponse {
    peer_id: String,
    device_name: String,
    code: String,
}

/// A pairing this device started.
#[derive(Serialize)]
struct OutgoingPairingResponse {
    peer_id: String,
    code: String,
    locally_confirmed: bool,
}

/// A pairing step's result, tagged by `kind`.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PairingUpdateResponse {
    IncomingRequest {
        peer_id: String,
        device_name: String,
        code: String,
    },
    AwaitingRemote {
        peer_id: String,
    },
    Paired {
        peer_id: String,
        device_name: String,
    },
    Rejected {
        peer_id: String,
    },
    Failed {
        peer_id: String,
    },
}

impl From<&PairingUpdate> for PairingUpdateResponse {
    fn from(update: &PairingUpdate) -> Self {
        match update {
            PairingUpdate::IncomingRequest {
                peer_id,
                device_name,
                code,
            } => Self::IncomingRequest {
                peer_id: peer_id.as_str().to_owned(),
                device_name: device_name.as_str().to_owned(),
                code: code.as_str().to_owned(),
            },
            PairingUpdate::AwaitingRemote { peer_id } => Self::AwaitingRemote {
                peer_id: peer_id.as_str().to_owned(),
            },
            PairingUpdate::Paired {
                peer_id,
                device_name,
            } => Self::Paired {
                peer_id: peer_id.as_str().to_owned(),
                device_name: device_name.as_str().to_owned(),
            },
            PairingUpdate::Rejected { peer_id } => Self::Rejected {
                peer_id: peer_id.as_str().to_owned(),
            },
            PairingUpdate::Failed { peer_id } => Self::Failed {
                peer_id: peer_id.as_str().to_owned(),
            },
        }
    }
}

/// Returns the snapshot, or `{"enabled": false}` without a P2P task.
async fn status(State(application_state): State<ServerAppState>) -> Json<StatusResponse> {
    let Some(control) = application_state.p2p_control() else {
        return Json(StatusResponse {
            enabled: false,
            details: None,
        });
    };
    let snapshot = control.snapshot();
    let status = snapshot.status;
    Json(StatusResponse {
        enabled: true,
        details: Some(StatusDetails {
            local_peer_id: status.local_peer_id.as_str().to_owned(),
            device_name: status.device_name.as_str().to_owned(),
            listen_addresses: snapshot.listen_addresses,
            trusted_devices: status
                .trusted_devices
                .iter()
                .map(|device| TrustedDeviceResponse {
                    peer_id: device.peer_id().as_str().to_owned(),
                    device_name: device.device_name().as_str().to_owned(),
                    paired_at_ms: device.paired_at_ms(),
                })
                .collect(),
            nearby_devices: status
                .nearby_devices
                .iter()
                .map(|peer_id| NearbyDeviceResponse {
                    peer_id: peer_id.as_str().to_owned(),
                })
                .collect(),
            incoming_requests: status
                .incoming_requests
                .iter()
                .map(|request| IncomingRequestResponse {
                    peer_id: request.peer_id.as_str().to_owned(),
                    device_name: request.device_name.as_str().to_owned(),
                    code: request.code.as_str().to_owned(),
                })
                .collect(),
            outgoing_pairings: status
                .outgoing_pairings
                .iter()
                .map(|pairing| OutgoingPairingResponse {
                    peer_id: pairing.peer_id.as_str().to_owned(),
                    code: pairing.code.as_str().to_owned(),
                    locally_confirmed: pairing.locally_confirmed,
                })
                .collect(),
            recent_updates: status
                .recent_updates
                .iter()
                .map(PairingUpdateResponse::from)
                .collect(),
        }),
    })
}

/// Opens the pairing window.
async fn make_pairable(
    State(application_state): State<ServerAppState>,
) -> Result<StatusCode, P2pRouteFailure> {
    run(&application_state, P2pCommand::MakePairable).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Starts pairing with a nearby device and returns the code.
async fn start_pairing(
    State(application_state): State<ServerAppState>,
    Json(request): Json<StartPairingRequest>,
) -> Result<Json<PairingCodeResponse>, P2pRouteFailure> {
    let peer_id = parse_peer_id(&request.peer_id)?;
    match run(&application_state, P2pCommand::StartPairing(peer_id)).await? {
        P2pCommandOutcome::PairingStarted(code) => Ok(Json(PairingCodeResponse {
            code: code.as_str().to_owned(),
        })),
        _ => Err(P2pRouteFailure::UnexpectedOutcome),
    }
}

/// Confirms matching codes for a pairing this device started.
async fn confirm_code(
    State(application_state): State<ServerAppState>,
    Path(peer_id): Path<String>,
) -> Result<Json<PairingUpdateResponse>, P2pRouteFailure> {
    let command = P2pCommand::ConfirmCode(parse_peer_id(&peer_id)?);
    pairing_update(run(&application_state, command).await?)
}

/// Abandons a pairing this device started.
async fn cancel_pairing(
    State(application_state): State<ServerAppState>,
    Path(peer_id): Path<String>,
) -> Result<StatusCode, P2pRouteFailure> {
    let command = P2pCommand::CancelPairing(parse_peer_id(&peer_id)?);
    run(&application_state, command).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Accepts another device's request after the owner compared codes.
async fn accept_request(
    State(application_state): State<ServerAppState>,
    Path(peer_id): Path<String>,
) -> Result<Json<PairingUpdateResponse>, P2pRouteFailure> {
    let command = P2pCommand::AcceptIncoming(parse_peer_id(&peer_id)?);
    pairing_update(run(&application_state, command).await?)
}

/// Declines another device's request.
async fn reject_request(
    State(application_state): State<ServerAppState>,
    Path(peer_id): Path<String>,
) -> Result<StatusCode, P2pRouteFailure> {
    let command = P2pCommand::RejectIncoming(parse_peer_id(&peer_id)?);
    run(&application_state, command).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Removes a paired device.
async fn revoke_device(
    State(application_state): State<ServerAppState>,
    Path(peer_id): Path<String>,
) -> Result<StatusCode, P2pRouteFailure> {
    let command = P2pCommand::Revoke(parse_peer_id(&peer_id)?);
    match run(&application_state, command).await? {
        P2pCommandOutcome::Revoked(true) => Ok(StatusCode::NO_CONTENT),
        P2pCommandOutcome::Revoked(false) => Err(P2pRouteFailure::NotPaired),
        _ => Err(P2pRouteFailure::UnexpectedOutcome),
    }
}

/// Sends `command` to the P2P task.
///
/// Outputs: The outcome, or `Unavailable` / `Refused`.
/// Side effects: Those of the command.
async fn run(
    application_state: &ServerAppState,
    command: P2pCommand,
) -> Result<P2pCommandOutcome, P2pRouteFailure> {
    let control: Arc<dyn P2pControl> = application_state
        .p2p_control()
        .ok_or(P2pRouteFailure::Unavailable)?;
    control.command(command).await.map_err(|error| match error {
        P2pControlError::Refused(reason) => P2pRouteFailure::Refused(reason),
        P2pControlError::Stopped => P2pRouteFailure::Unavailable,
    })
}

/// Extracts the pairing update from a confirm or accept outcome.
fn pairing_update(
    outcome: P2pCommandOutcome,
) -> Result<Json<PairingUpdateResponse>, P2pRouteFailure> {
    match outcome {
        P2pCommandOutcome::Pairing(update) => Ok(Json(PairingUpdateResponse::from(&update))),
        _ => Err(P2pRouteFailure::UnexpectedOutcome),
    }
}

/// Accepts 1–`MAX_PEER_ID_CHARACTERS` ASCII letters and digits, the shape of libp2p peer IDs.
fn parse_peer_id(raw_peer_id: &str) -> Result<DeviceId, P2pRouteFailure> {
    let well_formed = !raw_peer_id.is_empty()
        && raw_peer_id.len() <= MAX_PEER_ID_CHARACTERS
        && raw_peer_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric());
    if well_formed {
        Ok(DeviceId::new(raw_peer_id))
    } else {
        Err(P2pRouteFailure::BadPeerId)
    }
}
