//! Turns the Flutter Devices screen's JSON requests into P2P commands (Issue #43).
//!
//! `note_taker_p2p_status` and `note_taker_p2p_command` (in `ffi.rs`) call these functions with
//! the runtime's P2P task, or `None` when P2P is disabled or did not start. The status JSON is the
//! same as `GET /api/p2p/status` (`application::p2p_view`). A command is one JSON object tagged by
//! `command`:
//!
//! | `command` | Fields | Success |
//! | --- | --- | --- |
//! | `make_pairable` | none | `{"ok":true}` |
//! | `start_pairing` | `peer_id` | `{"ok":true,"code":"123456"}` |
//! | `confirm_code` | `peer_id` | `{"ok":true,"update":{"kind":...}}` |
//! | `cancel_pairing` | `peer_id` | `{"ok":true}` |
//! | `accept_request` | `peer_id` | `{"ok":true,"update":{"kind":...}}` |
//! | `reject_request` | `peer_id` | `{"ok":true}` |
//! | `revoke` | `peer_id` | `{"ok":true,"revoked":true|false}` |
//! | `set_transcription_route` | `route` (`local`, `auto`, or a peer ID) | `{"ok":true}` |
//! | `set_network_addresses` | `relay` (optional), `peers` | `{"ok":true}` |
//! | `create_invite` | none | `{"ok":true,"invite":"note-taker-invite:…","expires_at_ms":…}` |
//! | `join_invite` | `invite` | `{"ok":true,"peer_id":…}` |
//!
//! Failures are `{"ok":false,"error_code":...,"error":...}`, where `error_code` is
//! `invalid_request` (unreadable JSON, an unknown command or field, a missing `peer_id`),
//! `invalid_peer_id`, `unavailable` (no P2P task), or `refused` (the task declined, for example a
//! pairing that is not in progress). The network commands add the codes of
//! `NetworkCommandError`, such as `invalid_relay_address` or `invite_expired`. Nothing here logs peer IDs or device names.
//!
//! Commands block the calling thread until the task answers, so Dart calls them on a worker
//! isolate, like every other FFI call.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::application::{
    p2p_service::{
        NetworkCommand, NetworkCommandOutcome, P2pCommand, P2pCommandOutcome, P2pControl,
        P2pControlError,
    },
    p2p_view::{PairingUpdateView, parse_peer_id, parse_route_request, status_view},
};
use crate::domain::network_addresses::NetworkAddresses;

/// A command request, tagged by `command`.
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum CommandRequest {
    /// An empty struct variant, not a unit one: serde ignores `deny_unknown_fields` on unit
    /// variants of an internally tagged enum, so `{"command":"make_pairable","x":1}` would pass.
    MakePairable {},
    StartPairing {
        peer_id: String,
    },
    ConfirmCode {
        peer_id: String,
    },
    CancelPairing {
        peer_id: String,
    },
    AcceptRequest {
        peer_id: String,
    },
    RejectRequest {
        peer_id: String,
    },
    Revoke {
        peer_id: String,
    },
    SetTranscriptionRoute {
        route: String,
    },
    SetNetworkAddresses {
        relay: Option<String>,
        peers: Vec<String>,
    },
    CreateInvite {},
    JoinInvite {
        invite: String,
    },
}

/// Builds the failure object.
fn failure(error_code: &str, message: &str) -> Value {
    json!({ "ok": false, "error_code": error_code, "error": message })
}

/// Returns the status JSON: the running task's snapshot, or `{"enabled":false}`.
///
/// Inputs: The runtime's P2P task, if any.
/// Outputs: The JSON text.
/// Side effects: Reads the task's latest snapshot.
pub fn p2p_status_json(control: Option<&dyn P2pControl>) -> String {
    let snapshot = control.map(P2pControl::snapshot);
    serde_json::to_value(status_view(snapshot.as_ref()))
        .unwrap_or_else(|_| failure("internal", "the status could not be serialized"))
        .to_string()
}

/// Runs one JSON command against the task and returns the JSON answer.
///
/// Inputs: The runtime's P2P task, if any, and the request text from Dart.
/// Outputs: The success or failure object described in the module documentation.
/// Side effects: Those of the command (opening pairing, sending pairing messages, changing the
/// paired-device list). Blocks until the task answers.
pub fn p2p_command_json(control: Option<&dyn P2pControl>, request: &str) -> String {
    run_command(control, request).to_string()
}

/// Parses, validates, and runs `request`.
fn run_command(control: Option<&dyn P2pControl>, request: &str) -> Value {
    let request: CommandRequest = match serde_json::from_str(request) {
        Ok(request) => request,
        Err(_) => return failure("invalid_request", "the request is not a known P2P command"),
    };
    if let Some(command) = network_command_for(&request) {
        let Some(control) = control else {
            return failure("unavailable", "P2P sync is not running");
        };
        return match futures::executor::block_on(control.network_command(command)) {
            Ok(outcome) => network_success(outcome),
            Err(error) => failure(error.code(), &error.to_string()),
        };
    }
    let command = match command_for(request) {
        Some(command) => command,
        None => return failure("invalid_peer_id", "the peer ID is malformed"),
    };
    let Some(control) = control else {
        return failure("unavailable", "P2P sync is not running");
    };
    match futures::executor::block_on(control.command(command)) {
        Ok(outcome) => success(outcome),
        Err(P2pControlError::Refused(reason)) => failure("refused", &reason),
        Err(P2pControlError::Stopped) => failure("unavailable", "P2P sync is not running"),
    }
}

/// Maps a request to its command, or `None` for a malformed peer ID.
fn command_for(request: CommandRequest) -> Option<P2pCommand> {
    Some(match request {
        CommandRequest::MakePairable {} => P2pCommand::MakePairable,
        CommandRequest::StartPairing { peer_id } => {
            P2pCommand::StartPairing(parse_peer_id(&peer_id)?)
        }
        CommandRequest::ConfirmCode { peer_id } => {
            P2pCommand::ConfirmCode(parse_peer_id(&peer_id)?)
        }
        CommandRequest::CancelPairing { peer_id } => {
            P2pCommand::CancelPairing(parse_peer_id(&peer_id)?)
        }
        CommandRequest::AcceptRequest { peer_id } => {
            P2pCommand::AcceptIncoming(parse_peer_id(&peer_id)?)
        }
        CommandRequest::RejectRequest { peer_id } => {
            P2pCommand::RejectIncoming(parse_peer_id(&peer_id)?)
        }
        CommandRequest::Revoke { peer_id } => P2pCommand::Revoke(parse_peer_id(&peer_id)?),
        CommandRequest::SetTranscriptionRoute { route } => {
            P2pCommand::SetTranscriptionRoute(parse_route_request(&route)?)
        }
        // `network_command_for` takes these first, so they never get here.
        CommandRequest::SetNetworkAddresses { .. }
        | CommandRequest::CreateInvite {}
        | CommandRequest::JoinInvite { .. } => return None,
    })
}

/// Maps a request to its network command, or `None` for every other request.
///
/// Side effects: None. Blank relay and peer text counts as not set, and addresses are checked by
/// the P2P task.
fn network_command_for(request: &CommandRequest) -> Option<NetworkCommand> {
    match request {
        CommandRequest::SetNetworkAddresses { relay, peers } => {
            Some(NetworkCommand::SetAddresses(NetworkAddresses {
                relay: relay
                    .as_deref()
                    .map(str::trim)
                    .filter(|relay| !relay.is_empty())
                    .map(str::to_owned),
                peers: peers
                    .iter()
                    .map(|peer| peer.trim())
                    .filter(|peer| !peer.is_empty())
                    .map(str::to_owned)
                    .collect(),
            }))
        }
        CommandRequest::CreateInvite {} => Some(NetworkCommand::CreateInvite),
        CommandRequest::JoinInvite { invite } => Some(NetworkCommand::JoinInvite(invite.clone())),
        _ => None,
    }
}

/// Builds the success object for a network outcome.
fn network_success(outcome: NetworkCommandOutcome) -> Value {
    match outcome {
        NetworkCommandOutcome::Done => json!({ "ok": true }),
        NetworkCommandOutcome::Invite {
            text,
            expires_at_ms,
        } => json!({ "ok": true, "invite": text, "expires_at_ms": expires_at_ms }),
        NetworkCommandOutcome::Joined(peer_id) => {
            json!({ "ok": true, "peer_id": peer_id.as_str() })
        }
    }
}

/// Builds the success object for an outcome.
fn success(outcome: P2pCommandOutcome) -> Value {
    match outcome {
        P2pCommandOutcome::Done => json!({ "ok": true }),
        P2pCommandOutcome::PairingStarted(code) => json!({ "ok": true, "code": code.as_str() }),
        P2pCommandOutcome::Pairing(update) => {
            match serde_json::to_value(PairingUpdateView::from(&update)) {
                Ok(update) => json!({ "ok": true, "update": update }),
                Err(_) => failure("internal", "the pairing update could not be serialized"),
            }
        }
        P2pCommandOutcome::Revoked(revoked) => json!({ "ok": true, "revoked": revoked }),
    }
}

#[cfg(test)]
#[path = "ffi_p2p_tests.rs"]
mod tests;
