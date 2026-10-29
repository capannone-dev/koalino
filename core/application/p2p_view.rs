//! Serializable views of the P2P state, shared by the HTTP routes and the Flutter FFI (Issue #43).
//!
//! `GET /api/p2p/status` and `note_taker_p2p_status` return the same JSON, and both accept peer IDs
//! through `parse_peer_id`, so the web and Flutter Devices screens read one shape.

use serde::Serialize;

use crate::{
    application::{p2p_service::P2pSnapshot, pairing_manager::PairingUpdate},
    domain::models::{DeviceId, DeviceManifest},
};

/// Longest accepted peer ID; libp2p Ed25519 peer IDs are 52 characters.
pub const MAX_PEER_ID_CHARACTERS: usize = 128;

/// Accepts 1–`MAX_PEER_ID_CHARACTERS` ASCII letters and digits, the shape of libp2p peer IDs.
///
/// Inputs: Untrusted text from a request.
/// Outputs: The peer ID, or `None` when it is malformed (so it never reaches the P2P task).
/// Side effects: None.
pub fn parse_peer_id(raw_peer_id: &str) -> Option<DeviceId> {
    let well_formed = !raw_peer_id.is_empty()
        && raw_peer_id.len() <= MAX_PEER_ID_CHARACTERS
        && raw_peer_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric());
    well_formed.then(|| DeviceId::new(raw_peer_id))
}

/// The P2P state shown by the Devices screens: `GET /api/p2p/status` and `note_taker_p2p_status`.
#[derive(Serialize)]
pub struct P2pStatusView {
    /// Whether the P2P task runs; every other field is absent when it does not.
    pub enabled: bool,
    #[serde(flatten)]
    pub details: Option<P2pStatusDetails>,
}

/// The running task's state.
#[derive(Serialize)]
pub struct P2pStatusDetails {
    pub local_peer_id: String,
    pub device_name: String,
    pub listen_addresses: Vec<String>,
    /// What this device advertises to its paired devices (REQUIREMENTS 3.4).
    pub capabilities: CapabilitiesView,
    pub trusted_devices: Vec<TrustedDeviceView>,
    pub nearby_devices: Vec<NearbyDeviceView>,
    pub incoming_requests: Vec<IncomingRequestView>,
    pub outgoing_pairings: Vec<OutgoingPairingView>,
    pub recent_updates: Vec<PairingUpdateView>,
}

/// A paired device.
#[derive(Serialize)]
pub struct TrustedDeviceView {
    pub peer_id: String,
    pub device_name: String,
    pub paired_at_ms: u64,
    /// The device's manifest while it is connected; `None` (JSON `null`) otherwise.
    pub capabilities: Option<CapabilitiesView>,
}

/// A device's advertised capacity, from its `DeviceManifest`.
#[derive(Serialize)]
pub struct CapabilitiesView {
    pub cpu_cores: u16,
    pub gpu_vram_megabytes: u32,
    /// Model labels such as `whisper` or `openai`.
    pub transcription_models: Vec<String>,
    pub supports_vad: bool,
}

impl From<&DeviceManifest> for CapabilitiesView {
    fn from(manifest: &DeviceManifest) -> Self {
        Self {
            cpu_cores: manifest.cpu_cores(),
            gpu_vram_megabytes: manifest.gpu_vram_megabytes(),
            transcription_models: manifest
                .available_transcription_models()
                .iter()
                .map(|model| model.as_str().to_owned())
                .collect(),
            supports_vad: manifest.supports_vad,
        }
    }
}

/// An unpaired device on the local network.
#[derive(Serialize)]
pub struct NearbyDeviceView {
    pub peer_id: String,
}

/// A request awaiting the owner's answer.
#[derive(Serialize)]
pub struct IncomingRequestView {
    pub peer_id: String,
    pub device_name: String,
    pub code: String,
}

/// A pairing this device started.
#[derive(Serialize)]
pub struct OutgoingPairingView {
    pub peer_id: String,
    pub code: String,
    pub locally_confirmed: bool,
}

/// A pairing step's result, tagged by `kind`.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PairingUpdateView {
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

impl From<&PairingUpdate> for PairingUpdateView {
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

/// Builds the status view of a running task, or `{"enabled": false}` without one.
///
/// Inputs: The task's latest snapshot, or `None` when P2P is disabled or did not start.
/// Outputs: The view; it holds device names and peer IDs but no meeting content.
/// Side effects: None.
pub fn status_view(snapshot: Option<&P2pSnapshot>) -> P2pStatusView {
    let Some(snapshot) = snapshot else {
        return P2pStatusView {
            enabled: false,
            details: None,
        };
    };
    let status = &snapshot.status;
    P2pStatusView {
        enabled: true,
        details: Some(P2pStatusDetails {
            local_peer_id: status.local_peer_id.as_str().to_owned(),
            device_name: status.device_name.as_str().to_owned(),
            listen_addresses: snapshot.listen_addresses.clone(),
            capabilities: CapabilitiesView::from(&status.local_manifest),
            trusted_devices: status
                .trusted_devices
                .iter()
                .map(|device| TrustedDeviceView {
                    peer_id: device.peer_id().as_str().to_owned(),
                    device_name: device.device_name().as_str().to_owned(),
                    paired_at_ms: device.paired_at_ms(),
                    capabilities: status
                        .peer_manifests
                        .iter()
                        .find(|manifest| manifest.device_id() == device.peer_id())
                        .map(CapabilitiesView::from),
                })
                .collect(),
            nearby_devices: status
                .nearby_devices
                .iter()
                .map(|peer_id| NearbyDeviceView {
                    peer_id: peer_id.as_str().to_owned(),
                })
                .collect(),
            incoming_requests: status
                .incoming_requests
                .iter()
                .map(|request| IncomingRequestView {
                    peer_id: request.peer_id.as_str().to_owned(),
                    device_name: request.device_name.as_str().to_owned(),
                    code: request.code.as_str().to_owned(),
                })
                .collect(),
            outgoing_pairings: status
                .outgoing_pairings
                .iter()
                .map(|pairing| OutgoingPairingView {
                    peer_id: pairing.peer_id.as_str().to_owned(),
                    code: pairing.code.as_str().to_owned(),
                    locally_confirmed: pairing.locally_confirmed,
                })
                .collect(),
            recent_updates: status
                .recent_updates
                .iter()
                .map(PairingUpdateView::from)
                .collect(),
        }),
    }
}
