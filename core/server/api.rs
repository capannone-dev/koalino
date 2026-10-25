//! Implements the dependency-injected Axum REST and WebSocket adapter for headless mode.
//!
//! Handlers translate only port-returned domain values into JSON DTOs. SQLite, vector, CRDT, and
//! raw-audio details remain behind their adapters and are never accessed from HTTP code.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
};
use serde::Serialize;
use tokio::sync::{Mutex, broadcast};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    domain::{
        errors::StorageError,
        models::{AggregatedStats, Attachment, MeetingId, SpeakerIdentity, UnassignedVoiceClip},
    },
    ports::{
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
        voice::VoiceVectorDB,
    },
};

/// Combines the repository capabilities required by the headless read-only server boundary.
pub trait ServerRepository:
    NoteRepository + AttachmentRepository + MetricsRepository + Send
{
}

impl<T> ServerRepository for T where
    T: NoteRepository + AttachmentRepository + MetricsRepository + Send
{
}

/// Holds shared port implementations and the process-local status event broadcaster.
#[derive(Clone)]
pub struct ServerAppState {
    repositories: Arc<Mutex<Box<dyn ServerRepository>>>,
    voice_database: Arc<Mutex<Box<dyn VoiceVectorDB + Send>>>,
    event_publisher: broadcast::Sender<WebSocketEvent>,
}

impl ServerAppState {
    /// Creates state from adapter-neutral port implementations owned by the headless process.
    ///
    /// Inputs: `repositories` implements meeting, attachment, and telemetry ports; `voice_database`
    /// implements the voice-vector port.
    /// Outputs: Cloneable router state with a bounded real-time event channel.
    /// Side effects: Allocates a bounded process-local event broadcaster.
    pub fn new<RepositoryPort, VoiceDatabasePort>(
        repositories: RepositoryPort,
        voice_database: VoiceDatabasePort,
    ) -> Self
    where
        RepositoryPort: ServerRepository + 'static,
        VoiceDatabasePort: VoiceVectorDB + Send + 'static,
    {
        let (event_publisher, _) = broadcast::channel(32);
        Self {
            repositories: Arc::new(Mutex::new(Box::new(repositories))),
            voice_database: Arc::new(Mutex::new(Box::new(voice_database))),
            event_publisher,
        }
    }

    /// Publishes a non-sensitive lifecycle/status event to currently connected web clients.
    ///
    /// Inputs: `event` contains opaque IDs and status labels only.
    /// Outputs: None; no-receiver conditions are expected and intentionally ignored.
    /// Side effects: Delivers to current WebSocket receiver queues.
    pub fn publish_event(&self, event: WebSocketEvent) {
        let _ = self.event_publisher.send(event);
    }
}

/// Describes a real-time update safe to expose to the local web UI.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum WebSocketEvent {
    /// Indicates a locally created meeting entered recording lifecycle.
    MeetingStarted { meeting_id: String },
    /// Indicates portable P2P synchronization state changed without exposing peer payloads.
    SyncStatusChanged { status: String },
    /// Indicates a review item was created without exposing raw temporary audio.
    NewUnassignedClip { meeting_id: String, clip_id: String },
}

/// Builds the complete version-one headless HTTP router from injected state.
///
/// Inputs: `application_state` provides repository ports and status-event publication.
/// Outputs: A configured Axum router with local web-development CORS and request tracing.
/// Side effects: Registers HTTP/WebSocket handlers; it neither binds a socket nor opens storage.
pub fn build_router(application_state: ServerAppState) -> Router {
    Router::new()
        .route("/api/meetings", get(list_meetings))
        .route("/api/meetings/{meeting_id}", get(get_meeting))
        .route("/api/voice-gallery", get(get_voice_gallery))
        .route("/api/telemetry", get(get_telemetry))
        .route("/ws", get(web_socket_upgrade))
        .layer(
            CorsLayer::new()
                .allow_origin(HeaderValue::from_static("http://localhost:3000"))
                .allow_methods([axum::http::Method::GET]),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(application_state)
}

/// Returns display-safe metadata for every local meeting.
async fn list_meetings(
    State(application_state): State<ServerAppState>,
) -> Result<Json<Vec<MeetingSummary>>, StatusCode> {
    tracing::info!(endpoint = "/api/meetings", "headless API request received");
    let repositories = application_state.repositories.lock().await;
    let meeting_ids = repositories
        .list_meeting_ids()
        .map_err(storage_error_status)?;
    Ok(Json(
        meeting_ids.into_iter().map(MeetingSummary::from).collect(),
    ))
}

/// Returns one meeting's transcript and linked metadata-only attachments.
async fn get_meeting(
    Path(meeting_identifier): Path<String>,
    State(application_state): State<ServerAppState>,
) -> Result<Json<MeetingDetail>, StatusCode> {
    tracing::info!(endpoint = "/api/meetings/:meeting_id", meeting_id = %meeting_identifier, "headless API request received");
    let meeting_id = MeetingId::new(meeting_identifier);
    let repositories = application_state.repositories.lock().await;
    let transcript = repositories
        .load_transcript(&meeting_id)
        .map_err(storage_error_status)?;
    let attachments = repositories
        .get_attachments_for_meeting(&meeting_id)
        .map_err(storage_error_status)?;
    Ok(Json(MeetingDetail {
        meeting_id: String::from(meeting_id.as_str()),
        transcript: transcript.map(|value| String::from(value.content())),
        attachments: attachments.iter().map(AttachmentSummary::from).collect(),
    }))
}

/// Returns speaker labels and metadata-only unresolved review items for the local gallery.
async fn get_voice_gallery(
    State(application_state): State<ServerAppState>,
) -> Result<Json<VoiceGalleryResponse>, StatusCode> {
    tracing::info!(
        endpoint = "/api/voice-gallery",
        "headless API request received"
    );
    let identities = application_state
        .voice_database
        .lock()
        .await
        .list_speaker_identities()
        .map_err(storage_error_status)?;
    let repositories = application_state.repositories.lock().await;
    let meeting_ids = repositories
        .list_meeting_ids()
        .map_err(storage_error_status)?;
    let mut clips = Vec::new();
    for meeting_id in meeting_ids {
        clips.extend(
            repositories
                .get_unassigned_clips(&meeting_id)
                .map_err(storage_error_status)?,
        );
    }
    Ok(Json(VoiceGalleryResponse {
        identities: identities.iter().map(IdentitySummary::from).collect(),
        unassigned_clips: clips.iter().map(UnassignedClipSummary::from).collect(),
    }))
}

/// Returns aggregated privacy-safe AI timing metrics.
async fn get_telemetry(
    State(application_state): State<ServerAppState>,
) -> Result<Json<TelemetryResponse>, StatusCode> {
    tracing::info!(endpoint = "/api/telemetry", "headless API request received");
    let repositories = application_state.repositories.lock().await;
    Ok(Json(TelemetryResponse::from(
        repositories
            .get_aggregated_stats()
            .map_err(storage_error_status)?,
    )))
}

/// Upgrades a local client connection and streams event-bus messages until disconnect.
async fn web_socket_upgrade(
    web_socket_upgrade: WebSocketUpgrade,
    State(application_state): State<ServerAppState>,
) -> impl IntoResponse {
    tracing::info!("headless WebSocket connection requested");
    let event_receiver = application_state.event_publisher.subscribe();
    web_socket_upgrade.on_upgrade(move |web_socket| stream_events(web_socket, event_receiver))
}

/// Serializes event-bus updates as text frames without retaining client or event payload state.
async fn stream_events(
    mut web_socket: WebSocket,
    mut event_receiver: broadcast::Receiver<WebSocketEvent>,
) {
    tracing::info!("headless WebSocket connection established");
    while let Ok(event) = event_receiver.recv().await {
        let event_json = match serde_json::to_string(&event) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(error = %error, "unable to serialize WebSocket event");
                continue;
            }
        };
        if web_socket
            .send(Message::Text(event_json.into()))
            .await
            .is_err()
        {
            tracing::info!("headless WebSocket client disconnected");
            break;
        }
    }
}

/// Maps repository failures to stable HTTP responses without exposing adapter details.
fn storage_error_status(storage_error: StorageError) -> StatusCode {
    match storage_error {
        StorageError::Rejected { reason } if reason.contains("does not exist") => {
            StatusCode::NOT_FOUND
        }
        StorageError::Unavailable | StorageError::Rejected { .. } => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Serializes safe meeting-index data.
#[derive(Serialize)]
struct MeetingSummary {
    meeting_id: String,
}
impl From<MeetingId> for MeetingSummary {
    fn from(meeting_id: MeetingId) -> Self {
        Self {
            meeting_id: String::from(meeting_id.as_str()),
        }
    }
}

/// Serializes a complete meeting response without CRDT data or attachment content.
#[derive(Serialize)]
struct MeetingDetail {
    meeting_id: String,
    transcript: Option<String>,
    attachments: Vec<AttachmentSummary>,
}

/// Serializes metadata for one linked attachment.
#[derive(Serialize)]
struct AttachmentSummary {
    attachment_id: String,
    file_path: String,
    mime_type: String,
    original_timestamp_utc: String,
}
impl From<&Attachment> for AttachmentSummary {
    fn from(attachment: &Attachment) -> Self {
        Self {
            attachment_id: attachment.attachment_id().to_string(),
            file_path: String::from(attachment.file_path()),
            mime_type: String::from(attachment.mime_type()),
            original_timestamp_utc: attachment.original_timestamp().to_rfc3339(),
        }
    }
}

/// Serializes a gallery response without voice embeddings or raw audio bytes.
#[derive(Serialize)]
struct VoiceGalleryResponse {
    identities: Vec<IdentitySummary>,
    unassigned_clips: Vec<UnassignedClipSummary>,
}
#[derive(Serialize)]
struct IdentitySummary {
    identity_id: String,
    display_name: String,
}
impl From<&SpeakerIdentity> for IdentitySummary {
    fn from(identity: &SpeakerIdentity) -> Self {
        Self {
            identity_id: identity.identity_id().to_string(),
            display_name: String::from(identity.display_name()),
        }
    }
}
#[derive(Serialize)]
struct UnassignedClipSummary {
    clip_id: String,
    meeting_id: String,
    start_time_ms: u64,
    end_time_ms: u64,
}
impl From<&UnassignedVoiceClip> for UnassignedClipSummary {
    fn from(clip: &UnassignedVoiceClip) -> Self {
        Self {
            clip_id: clip.clip_id().to_string(),
            meeting_id: String::from(clip.meeting_id().as_str()),
            start_time_ms: clip.start_time_ms(),
            end_time_ms: clip.end_time_ms(),
        }
    }
}

/// Serializes aggregate metrics with no raw AI task payloads.
#[derive(Serialize)]
struct TelemetryResponse {
    task_count: u64,
    total_duration_ms: u64,
    average_latency_ms: u64,
}
impl From<AggregatedStats> for TelemetryResponse {
    fn from(stats: AggregatedStats) -> Self {
        Self {
            task_count: stats.task_count(),
            total_duration_ms: stats.total_duration_ms(),
            average_latency_ms: stats.average_latency_ms(),
        }
    }
}
