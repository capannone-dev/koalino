//! Encodes the `NTP1` messages that copy the project list to a paired device (Issue #85).
//!
//! A message is `NTP1` followed by a JSON body sealed with the pair key (`seal_for_peer`, purpose
//! `NTP1`, bound to the sender and receiver), holding every project and deletion record. Project
//! names are private, so they travel only inside the sealed value and are never logged. The sender
//! holds the pair key but is still peer input, so `decode_project_library_message` validates every
//! name and bounds the list.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    application::{
        crypto::pair_key::{open_from_peer, seal_for_peer},
        voice_library_exchange::rfc3339,
    },
    domain::{
        models::DeviceId,
        peer_trust::PairKey,
        projects::{Project, ProjectId, ProjectLibrary, ProjectName, ProjectTombstone},
    },
};

/// Marks a project-list message.
const PROJECT_LIBRARY_HEADER: &[u8; 4] = b"NTP1";
/// The largest message accepted. A project takes well under 1 KiB of JSON.
const MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
/// The most projects, or deletion records, one message may hold.
pub const MAX_PROJECTS: usize = 4_096;

/// Why a project-list message could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectLibraryMessageError {
    /// The message exceeds the size limit.
    #[error("project-list message is too large")]
    TooLarge,
    /// The message or its body cannot be parsed.
    #[error("project-list message is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("project-list message failed authentication")]
    Unauthenticated,
    /// The list holds unusable names or too many entries.
    #[error("project list holds implausible values")]
    Invalid,
    /// Encryption failed.
    #[error("project list could not be encrypted")]
    EncryptionFailed,
}

/// The JSON body of a message.
#[derive(Serialize, Deserialize)]
struct LibraryBody {
    projects: Vec<ProjectBody>,
    tombstones: Vec<TombstoneBody>,
}

/// One project in the body.
#[derive(Serialize, Deserialize)]
struct ProjectBody {
    project_id: Uuid,
    name: String,
    #[serde(with = "rfc3339")]
    created_at: DateTime<Utc>,
    #[serde(with = "rfc3339")]
    updated_at: DateTime<Utc>,
}

/// One deletion record in the body.
#[derive(Serialize, Deserialize)]
struct TombstoneBody {
    project_id: Uuid,
    #[serde(with = "rfc3339")]
    deleted_at: DateTime<Utc>,
}

/// Reports whether `payload` is a project-list message.
///
/// Inputs: A received payload. Outputs: Whether it starts with the header. Side effects: None.
pub fn is_project_library_message(payload: &[u8]) -> bool {
    payload.starts_with(PROJECT_LIBRARY_HEADER)
}

/// Seals `library` for `receiver`.
///
/// Inputs: The library, the pair key of the two devices, and both peer IDs.
/// Outputs: The sealed message, or `Invalid`, `TooLarge` or `EncryptionFailed`.
/// Side effects: Draws a random nonce.
pub fn encode_project_library_message(
    library: &ProjectLibrary,
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<Vec<u8>, ProjectLibraryMessageError> {
    if library.projects.len() > MAX_PROJECTS || library.tombstones.len() > MAX_PROJECTS {
        return Err(ProjectLibraryMessageError::Invalid);
    }
    let body = LibraryBody {
        projects: library
            .projects
            .iter()
            .map(|project| ProjectBody {
                project_id: project.project_id().as_uuid(),
                name: project.name().as_str().to_owned(),
                created_at: project.created_at(),
                updated_at: project.updated_at(),
            })
            .collect(),
        tombstones: library
            .tombstones
            .iter()
            .map(|tombstone| TombstoneBody {
                project_id: tombstone.project_id.as_uuid(),
                deleted_at: tombstone.deleted_at,
            })
            .collect(),
    };
    let plaintext = zeroize::Zeroizing::new(
        serde_json::to_vec(&body).map_err(|_| ProjectLibraryMessageError::Malformed)?,
    );
    let sealed = seal_for_peer(
        pair_key,
        PROJECT_LIBRARY_HEADER,
        sender,
        receiver,
        &plaintext,
    )
    .map_err(|_| ProjectLibraryMessageError::EncryptionFailed)?;
    let mut message = Vec::with_capacity(PROJECT_LIBRARY_HEADER.len() + sealed.len());
    message.extend_from_slice(PROJECT_LIBRARY_HEADER);
    message.extend_from_slice(&sealed);
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(ProjectLibraryMessageError::TooLarge);
    }
    Ok(message)
}

/// Opens and validates a message `sender` sent to `receiver`.
///
/// Inputs: A payload for which `is_project_library_message` holds, the pair key, and both peer
/// IDs. Outputs: The library, or why it was rejected (`Invalid` for a name `ProjectName` refuses
/// or an oversized list). Side effects: None; nothing is logged.
pub fn decode_project_library_message(
    payload: &[u8],
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<ProjectLibrary, ProjectLibraryMessageError> {
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(ProjectLibraryMessageError::TooLarge);
    }
    let sealed = payload
        .strip_prefix(PROJECT_LIBRARY_HEADER.as_slice())
        .ok_or(ProjectLibraryMessageError::Malformed)?;
    let plaintext = zeroize::Zeroizing::new(
        open_from_peer(pair_key, PROJECT_LIBRARY_HEADER, sender, receiver, sealed)
            .map_err(|_| ProjectLibraryMessageError::Unauthenticated)?,
    );
    let body: LibraryBody =
        serde_json::from_slice(&plaintext).map_err(|_| ProjectLibraryMessageError::Malformed)?;
    if body.projects.len() > MAX_PROJECTS || body.tombstones.len() > MAX_PROJECTS {
        return Err(ProjectLibraryMessageError::Invalid);
    }
    let projects = body
        .projects
        .into_iter()
        .map(|project| {
            ProjectName::parse(&project.name)
                .map(|name| {
                    Project::new(
                        ProjectId::from_uuid(project.project_id),
                        name,
                        project.created_at,
                        project.updated_at,
                    )
                })
                .map_err(|_| ProjectLibraryMessageError::Invalid)
        })
        .collect::<Result<Vec<Project>, _>>()?;
    Ok(ProjectLibrary {
        projects,
        tombstones: body
            .tombstones
            .into_iter()
            .map(|tombstone| ProjectTombstone {
                project_id: ProjectId::from_uuid(tombstone.project_id),
                deleted_at: tombstone.deleted_at,
            })
            .collect(),
    })
}
