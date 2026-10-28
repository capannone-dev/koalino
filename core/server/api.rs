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
    http::{HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, broadcast};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use super::request_guard::{DASHBOARD_ORIGIN, reject_cross_site_requests};
use crate::{
    adapters::export::default_exporters,
    application::{
        cluster_review::{
            ClusterReviewError, NamedClusterAssignment, UnknownClusterSummary,
            assign_cluster_to_named_identity, list_unknown_clusters,
        },
        export_manager::{ExportManager, ExportWriter},
        meeting_crdt::MeetingCrdtService,
        meeting_rename::{MeetingRenameError, rename_meeting},
        p2p_service::P2pControl,
        unknown_speaker_clusterer::ClusteringError,
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::{ExportError, StorageError},
        models::{
            AggregatedStats, Attachment, ExportFormat, ExportOptions, MeetingId, MeetingMetadata,
            MeetingTitle, SpeakerIdentity, TaskMetrics, TaskType, UnassignedVoiceClip,
        },
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
    /// The running P2P task, when P2P is enabled (Issue #43).
    p2p_control: Option<Arc<dyn P2pControl>>,
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
            p2p_control: None,
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

    /// Attaches the running P2P task, which the `/api/p2p/*` routes drive.
    ///
    /// Side effects: None; without it those routes report P2P as disabled.
    pub fn with_p2p_control(mut self, p2p_control: Arc<dyn P2pControl>) -> Self {
        self.p2p_control = Some(p2p_control);
        self
    }

    /// The attached P2P task, if any.
    pub(super) fn p2p_control(&self) -> Option<Arc<dyn P2pControl>> {
        self.p2p_control.clone()
    }

    /// Tells the P2P task, if any, that `meeting_id` was just written, so connected devices get it.
    ///
    /// Side effects: Queues a notification without waiting; nothing without P2P.
    fn notify_local_change(&self, meeting_id: &MeetingId) {
        if let Some(p2p_control) = &self.p2p_control {
            p2p_control.notify_local_change(meeting_id);
        }
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
    /// Indicates privacy-safe telemetry changed and dashboard clients should refresh their views.
    TelemetryUpdated,
}

/// Builds the complete version-one headless HTTP router from injected state.
///
/// Inputs: `application_state` provides repository ports and status-event publication.
/// Outputs: A configured Axum router with local web-development CORS and request tracing.
/// Side effects: Registers HTTP/WebSocket handlers; it neither binds a socket nor opens storage.
pub fn build_router(application_state: ServerAppState) -> Router {
    Router::new()
        .route("/api/meetings", get(list_meetings).post(create_meeting))
        .route(
            "/api/meetings/{meeting_id}",
            get(get_meeting).patch(rename_meeting_title),
        )
        .route("/api/meetings/{meeting_id}/export", post(export_meeting))
        .route(
            "/api/meetings/{meeting_id}/unknown-clusters",
            get(get_unknown_clusters),
        )
        .route("/api/clusters/{cluster_id}/assign", post(assign_cluster))
        .route("/api/voice-gallery", get(get_voice_gallery))
        .route("/api/voice/assign", post(assign_voice_clip))
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/devices", get(list_devices))
        .route(
            "/api/telemetry/devices/{device_id}",
            get(get_device_telemetry),
        )
        .route("/api/telemetry/summary", get(get_telemetry_summary))
        .route("/ws", get(web_socket_upgrade))
        .merge(super::p2p_routes::routes())
        // Innermost of the three layers, so CORS still answers preflights for allowed origins
        // and refusals are traced.
        .layer(middleware::from_fn(reject_cross_site_requests))
        .layer(
            CorsLayer::new()
                .allow_origin(HeaderValue::from_static(DASHBOARD_ORIGIN))
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PATCH,
                    axum::http::Method::DELETE,
                ])
                .allow_headers([header::CONTENT_TYPE])
                .expose_headers([header::CONTENT_DISPOSITION]),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(application_state)
}

/// Returns display-safe metadata, including the display title, for every local meeting.
async fn list_meetings(
    State(application_state): State<ServerAppState>,
) -> Result<Json<Vec<MeetingSummary>>, StatusCode> {
    tracing::info!(endpoint = "/api/meetings", "headless API request received");
    let repositories = application_state.repositories.lock().await;
    let meeting_ids = repositories
        .list_meeting_ids()
        .map_err(storage_error_status)?;
    let mut meeting_summaries = Vec::with_capacity(meeting_ids.len());
    for meeting_id in meeting_ids {
        if let Some(meeting_metadata) = repositories
            .load_meeting_metadata(&meeting_id)
            .map_err(storage_error_status)?
        {
            meeting_summaries.push(MeetingSummary::from(&meeting_metadata));
        }
    }
    Ok(Json(meeting_summaries))
}

/// Request body for `POST /api/meetings`; an omitted title leaves the meeting untitled.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateMeetingRequest {
    title: Option<String>,
}

/// Creates an empty, optionally titled meeting with a generated `meeting-<uuid>` ID.
///
/// Inputs: A JSON `CreateMeetingRequest`.
/// Outputs: `201 Created` with the meeting summary, `400` for an invalid title (nothing is
/// created), axum's `4xx` for a malformed body, or a storage error status.
/// Side effects: Inserts the meeting (with an empty CRDT document) and its title into SQLite.
/// The title is never logged.
async fn create_meeting(
    State(application_state): State<ServerAppState>,
    Json(request): Json<CreateMeetingRequest>,
) -> Result<(StatusCode, Json<MeetingSummary>), StatusCode> {
    tracing::info!(
        endpoint = "POST /api/meetings",
        "headless API request received"
    );
    let title = request
        .title
        .as_deref()
        .map(MeetingTitle::parse)
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let meeting_id = MeetingId::new(format!("meeting-{}", uuid::Uuid::new_v4()));
    let repositories = application_state.repositories.lock().await;
    let repository: &dyn ServerRepository = &**repositories;
    let crdt_service = MeetingCrdtService::new(repository);
    match &title {
        Some(title) => crdt_service.create_titled_meeting(&meeting_id, title),
        None => crdt_service.create_meeting(&meeting_id),
    }
    .map_err(storage_error_status)?;
    let meeting_metadata = repository
        .load_meeting_metadata(&meeting_id)
        .map_err(storage_error_status)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    application_state.notify_local_change(&meeting_id);
    Ok((
        StatusCode::CREATED,
        Json(MeetingSummary::from(&meeting_metadata)),
    ))
}

/// Returns one meeting's title, transcript, and linked metadata-only attachments.
async fn get_meeting(
    Path(meeting_identifier): Path<String>,
    State(application_state): State<ServerAppState>,
) -> Result<Json<MeetingDetail>, StatusCode> {
    tracing::info!(endpoint = "/api/meetings/:meeting_id", meeting_id = %meeting_identifier, "headless API request received");
    let meeting_id = MeetingId::new(meeting_identifier);
    let repositories = application_state.repositories.lock().await;
    let meeting_metadata = repositories
        .load_meeting_metadata(&meeting_id)
        .map_err(storage_error_status)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let transcript = repositories
        .load_transcript(&meeting_id)
        .map_err(storage_error_status)?;
    let attachments = repositories
        .get_attachments_for_meeting(&meeting_id)
        .map_err(storage_error_status)?;
    Ok(Json(MeetingDetail {
        meeting_id: String::from(meeting_id.as_str()),
        title: meeting_metadata.display_title(),
        created_at: meeting_metadata.created_at().to_rfc3339(),
        transcript: transcript.map(|value| String::from(value.content())),
        attachments: attachments.iter().map(AttachmentSummary::from).collect(),
    }))
}

/// Request body for `PATCH /api/meetings/{meeting_id}`; `title` is required.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameMeetingRequest {
    title: String,
}

/// Renames one meeting through the shared meeting-rename use case.
///
/// Inputs: The meeting ID path segment and a `{"title": "…"}` JSON body.
/// Outputs: `200` with the meeting summary (`meeting_id`, the trimmed `title`, `created_at`);
/// `400` for a blank, over-200-character, or control-character title; `404` for an unknown
/// meeting; `500` for storage failures. Errors are `{"error": "…"}`. A malformed body (missing
/// `title`, wrong type, unknown field) gets axum's `4xx`.
/// Side effects: Replaces the title in SQLite under the repository lock; validation happens
/// first, so a rejected title writes nothing. The title is never logged.
async fn rename_meeting_title(
    Path(meeting_identifier): Path<String>,
    State(application_state): State<ServerAppState>,
    Json(rename_request): Json<RenameMeetingRequest>,
) -> Response {
    tracing::info!(endpoint = "PATCH /api/meetings/:meeting_id", meeting_id = %meeting_identifier, "headless API request received");
    let meeting_id = MeetingId::new(meeting_identifier);
    let repositories = application_state.repositories.lock().await;
    let rename_result = rename_meeting(&**repositories, &meeting_id, &rename_request.title)
        .and_then(|_| {
            repositories
                .load_meeting_metadata(&meeting_id)
                .map_err(MeetingRenameError::from)
        });
    let (status_code, error) = match rename_result {
        Ok(Some(meeting_metadata)) => {
            application_state.notify_local_change(&meeting_id);
            return Json(MeetingSummary::from(&meeting_metadata)).into_response();
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            MeetingRenameError::MeetingNotFound { meeting_id }.to_string(),
        ),
        Err(error @ MeetingRenameError::InvalidTitle(_)) => {
            (StatusCode::BAD_REQUEST, error.to_string())
        }
        Err(error @ MeetingRenameError::MeetingNotFound { .. }) => {
            (StatusCode::NOT_FOUND, error.to_string())
        }
        Err(MeetingRenameError::Storage(storage_error)) => (
            storage_error_status(storage_error),
            String::from("unable to rename the meeting"),
        ),
    };
    (status_code, Json(ExportErrorBody { error })).into_response()
}

/// Request body for `POST /api/meetings/{meeting_id}/export`.
#[derive(Deserialize)]
struct ExportRequestBody {
    /// `markdown`/`md`, `pdf`, or `json`.
    format: String,
    /// Optional inclusion policy; omitted fields default to `true`.
    #[serde(default)]
    options: ExportRequestOptions,
}

/// Inclusion flags accepted by the export endpoint.
#[derive(Default, Deserialize)]
struct ExportRequestOptions {
    include_attachments: Option<bool>,
    include_metadata: Option<bool>,
}

/// JSON error body returned by the export, rename, and unknown-speaker cluster endpoints.
#[derive(Serialize)]
struct ExportErrorBody {
    error: String,
}

/// Exports one meeting and returns the rendered file as a download.
///
/// Inputs: The meeting ID path segment and an `ExportRequestBody` JSON body.
/// Outputs: `200` with the file bytes, `Content-Type` for the format, and a
/// `Content-Disposition: attachment` filename; `400` for unknown or unsupported formats; `404`
/// for unknown meetings; `500` for storage or rendering failures. Errors are `{"error": "…"}`.
/// Side effects: Reads repositories under their locks, then renders on a blocking thread into a
/// per-request temporary directory that is deleted before responding. Markdown downloads contain
/// only the `.md` file, so attachment links point to files that are not bundled.
async fn export_meeting(
    Path(meeting_identifier): Path<String>,
    State(application_state): State<ServerAppState>,
    Json(request_body): Json<ExportRequestBody>,
) -> Response {
    tracing::info!(endpoint = "/api/meetings/:meeting_id/export", meeting_id = %meeting_identifier, "headless API request received");
    let format = match request_body.format.parse::<ExportFormat>() {
        Ok(format) => format,
        Err(error) => return export_error_response(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let options = ExportOptions::new(
        format,
        request_body.options.include_attachments.unwrap_or(true),
        request_body.options.include_metadata.unwrap_or(true),
        None,
    );
    let request_directory = std::env::temp_dir()
        .join("note-taker-http-exports")
        .join(uuid::Uuid::new_v4().to_string());
    let export_writer = ExportWriter::new(default_exporters(), &request_directory);
    let document = {
        let repositories = application_state.repositories.lock().await;
        let voice_database = application_state.voice_database.lock().await;
        let export_manager = ExportManager::new(
            &**repositories,
            &**repositories,
            &**voice_database,
            &export_writer,
        );
        export_writer
            .ensure_format_supported(format)
            .map_err(anyhow::Error::from)
            .and_then(|()| {
                export_manager.extract_meeting(&MeetingId::new(meeting_identifier), &options)
            })
    };
    let document = match document {
        Ok(document) => document,
        Err(error) => return export_failure_response(&error),
    };
    let rendering_outcome = tokio::task::spawn_blocking(move || {
        let write_outcome = export_writer
            .write(vec![document], &options, None)
            .and_then(|export_result| {
                let file_bytes = std::fs::read(export_result.file_path())?;
                Ok((export_result, file_bytes))
            });
        let _ = std::fs::remove_dir_all(&request_directory);
        write_outcome
    })
    .await;
    match rendering_outcome {
        Ok(Ok((export_result, file_bytes))) => {
            let download_name = std::path::Path::new(export_result.file_path())
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap_or("export")
                .replace('"', "_");
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, export_content_type(format)),
                    (
                        header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{download_name}\""),
                    ),
                ],
                file_bytes,
            )
                .into_response()
        }
        Ok(Err(error)) => export_failure_response(&error),
        Err(_) => export_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            String::from("export task was cancelled"),
        ),
    }
}

/// Returns the HTTP media type for an export format.
fn export_content_type(format: ExportFormat) -> String {
    String::from(match format {
        ExportFormat::Markdown => "text/markdown; charset=utf-8",
        ExportFormat::Pdf => "application/pdf",
        ExportFormat::Json => "application/json",
        ExportFormat::Srt => "application/x-subrip",
    })
}

/// Maps an export failure to a status code, keeping internal details out of 500 responses.
fn export_failure_response(error: &anyhow::Error) -> Response {
    match error.downcast_ref::<ExportError>() {
        Some(ExportError::MeetingNotFound { .. }) => {
            export_error_response(StatusCode::NOT_FOUND, error.to_string())
        }
        Some(
            ExportError::UnknownFormat { .. }
            | ExportError::UnsupportedFormat { .. }
            | ExportError::InvalidDateRange,
        ) => export_error_response(StatusCode::BAD_REQUEST, error.to_string()),
        _ => {
            tracing::error!(error = %format!("{error:#}"), "meeting export failed");
            export_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("meeting export failed"),
            )
        }
    }
}

/// Builds a JSON error response.
fn export_error_response(status_code: StatusCode, message: String) -> Response {
    (status_code, Json(ExportErrorBody { error: message })).into_response()
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

/// Lists devices inferred from the privacy-safe telemetry rows known to this offline replica.
async fn list_devices(
    State(application_state): State<ServerAppState>,
) -> Result<Json<DevicesResponse>, StatusCode> {
    let repositories = application_state.repositories.lock().await;
    Ok(Json(DevicesResponse::from_metrics(
        repositories
            .get_all_metrics()
            .map_err(storage_error_status)?,
    )))
}

/// Returns one device's completed metrics from the last 24 hours without task content.
async fn get_device_telemetry(
    Path(device_id): Path<String>,
    State(application_state): State<ServerAppState>,
) -> Result<Json<DeviceTelemetryResponse>, StatusCode> {
    let repositories = application_state.repositories.lock().await;
    let minimum_timestamp_ms = current_epoch_ms().saturating_sub(86_400_000);
    let metrics: Vec<TaskMetrics> = repositories
        .get_metrics_for_device(&device_id)
        .map_err(storage_error_status)?
        .into_iter()
        .filter(|metric| metric.timestamp_epoch_ms() >= minimum_timestamp_ms)
        .collect();
    if metrics.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(DeviceTelemetryResponse {
        device_id,
        metrics: metrics.iter().map(MetricResponse::from).collect(),
    }))
}

/// Returns timing, latency, task-count, and coarse power aggregates for the dashboard.
async fn get_telemetry_summary(
    State(application_state): State<ServerAppState>,
) -> Result<Json<TelemetrySummaryResponse>, StatusCode> {
    let repositories = application_state.repositories.lock().await;
    Ok(Json(TelemetrySummaryResponse::from_metrics(
        repositories
            .get_all_metrics()
            .map_err(storage_error_status)?,
    )))
}

/// Resolves an unassigned clip to a user-selected identity through the voice application service.
async fn assign_voice_clip(
    State(application_state): State<ServerAppState>,
    Json(assignment_request): Json<VoiceAssignmentRequest>,
) -> Result<StatusCode, StatusCode> {
    let clip_id =
        uuid::Uuid::parse_str(&assignment_request.clip_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let identity_id = uuid::Uuid::parse_str(&assignment_request.identity_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    tracing::info!(clip_id = %clip_id, identity_id = %identity_id, "headless voice assignment requested");
    let review_clip = {
        let repositories = application_state.repositories.lock().await;
        let meeting_ids = repositories
            .list_meeting_ids()
            .map_err(storage_error_status)?;
        let mut matching_clip = None;
        for meeting_id in meeting_ids {
            let clips = repositories
                .get_unassigned_clips(&meeting_id)
                .map_err(storage_error_status)?;
            if let Some(clip) = clips.into_iter().find(|clip| clip.clip_id() == clip_id) {
                matching_clip = Some(clip);
                break;
            }
        }
        matching_clip.ok_or(StatusCode::NOT_FOUND)?
    };
    let voice_database = application_state.voice_database.lock().await;
    let voice_identity_manager = VoiceIdentityManager::new(&**voice_database);
    voice_identity_manager
        .confirm_weak_match(identity_id, review_clip.derived_embedding())
        .map_err(storage_error_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists one meeting's unresolved unknown-speaker clusters through the cluster-review use case.
///
/// Inputs: `meeting_id` path segment.
/// Outputs: `200` with a JSON array of `UnknownClusterResponse` in creation order (empty for a
/// meeting without clusters, including an unknown meeting), or `500` with `{"error": "…"}`.
/// Side effects: Reads cluster rows under the repository lock; embeddings are never serialized.
async fn get_unknown_clusters(
    Path(meeting_identifier): Path<String>,
    State(application_state): State<ServerAppState>,
) -> Response {
    tracing::info!(endpoint = "/api/meetings/:meeting_id/unknown-clusters", meeting_id = %meeting_identifier, "headless API request received");
    let repositories = application_state.repositories.lock().await;
    match list_unknown_clusters(&**repositories, &MeetingId::new(meeting_identifier)) {
        Ok(summaries) => Json(
            summaries
                .iter()
                .map(UnknownClusterResponse::from)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => cluster_review_error_response(&error),
    }
}

/// Assigns an unknown-speaker cluster to a named identity through the cluster-review use case.
///
/// Inputs: `cluster_id` path segment (a UUID; anything else is rejected with `400` by Axum) and a
/// `{"identity_name": "…"}` body.
/// Outputs: `200` with a `ClusterAssignmentResponse`; `400` for a blank name; `404` for an unknown
/// cluster; `409` for an already-resolved cluster; `500` for storage failures. Errors are
/// `{"error": "…"}`.
/// Side effects: Under the repository and voice locks, stores voice variants, relinks transcript
/// segments, resolves the cluster, and deletes its review clips.
async fn assign_cluster(
    Path(cluster_id): Path<uuid::Uuid>,
    State(application_state): State<ServerAppState>,
    Json(assignment_request): Json<ClusterAssignmentRequest>,
) -> Response {
    tracing::info!(endpoint = "/api/clusters/:cluster_id/assign", cluster_id = %cluster_id, "headless API request received");
    let repositories = application_state.repositories.lock().await;
    let voice_database = application_state.voice_database.lock().await;
    match assign_cluster_to_named_identity(
        &**repositories,
        &**voice_database,
        cluster_id,
        &assignment_request.identity_name,
    ) {
        Ok(assignment) => Json(ClusterAssignmentResponse::from(&assignment)).into_response(),
        Err(error) => cluster_review_error_response(&error),
    }
}

/// Maps cluster-review failures to stable HTTP statuses with a `{"error": "…"}` body.
fn cluster_review_error_response(error: &ClusterReviewError) -> Response {
    let status_code = match error {
        ClusterReviewError::BlankIdentityName => StatusCode::BAD_REQUEST,
        ClusterReviewError::Clustering(ClusteringError::ClusterNotFound { .. }) => {
            StatusCode::NOT_FOUND
        }
        ClusterReviewError::Clustering(ClusteringError::ClusterAlreadyResolved { .. }) => {
            StatusCode::CONFLICT
        }
        ClusterReviewError::Clustering(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status_code,
        Json(ExportErrorBody {
            error: error.to_string(),
        }),
    )
        .into_response()
}

/// Request body for `POST /api/clusters/{cluster_id}/assign`.
#[derive(Deserialize)]
struct ClusterAssignmentRequest {
    /// Person's name; a known identity with this name is reused, otherwise one is created.
    identity_name: String,
}

/// Serializes one unresolved cluster without its representative embedding.
#[derive(Serialize)]
struct UnknownClusterResponse {
    cluster_id: String,
    meeting_id: String,
    total_duration_ms: u64,
    chunk_count: usize,
    /// RFC 3339 UTC creation time of the cluster's first chunk.
    created_at: String,
}

impl From<&UnknownClusterSummary> for UnknownClusterResponse {
    fn from(summary: &UnknownClusterSummary) -> Self {
        Self {
            cluster_id: summary.cluster_id().to_string(),
            meeting_id: String::from(summary.meeting_id().as_str()),
            total_duration_ms: summary.total_duration_ms(),
            chunk_count: summary.chunk_count(),
            created_at: summary.created_at().to_rfc3339(),
        }
    }
}

/// Serializes the outcome of a cluster assignment.
#[derive(Serialize)]
struct ClusterAssignmentResponse {
    cluster_id: String,
    identity_id: String,
    identity_name: String,
    /// `true` when the identity had no voice variants before this assignment.
    identity_created: bool,
    variants_stored: usize,
    segments_updated: usize,
}

impl From<&NamedClusterAssignment> for ClusterAssignmentResponse {
    fn from(assignment: &NamedClusterAssignment) -> Self {
        Self {
            cluster_id: assignment.cluster_id().to_string(),
            identity_id: assignment.identity_id().to_string(),
            identity_name: String::from(assignment.identity_name()),
            identity_created: assignment.identity_created(),
            variants_stored: assignment.variants_stored(),
            segments_updated: assignment.segments_updated(),
        }
    }
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
    /// The stored title, or the dated fallback for an untitled meeting.
    title: String,
    /// RFC 3339 UTC creation time.
    created_at: String,
}
impl From<&MeetingMetadata> for MeetingSummary {
    fn from(meeting_metadata: &MeetingMetadata) -> Self {
        Self {
            meeting_id: String::from(meeting_metadata.meeting_id().as_str()),
            title: meeting_metadata.display_title(),
            created_at: meeting_metadata.created_at().to_rfc3339(),
        }
    }
}

/// Serializes a complete meeting response without CRDT data or attachment content.
#[derive(Serialize)]
struct MeetingDetail {
    meeting_id: String,
    /// The stored title, or the dated fallback for an untitled meeting.
    title: String,
    /// RFC 3339 UTC creation time.
    created_at: String,
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

/// Serializes device observations inferred from locally persisted telemetry rows.
#[derive(Serialize)]
struct DevicesResponse {
    devices: Vec<DeviceResponse>,
}

/// Serializes one dashboard device card without hardware serial numbers or task content.
#[derive(Serialize)]
struct DeviceResponse {
    device_id: String,
    supports_vad: bool,
    supported_models: Vec<String>,
    last_seen: u64,
}

impl DevicesResponse {
    /// Groups persisted metrics into one display-safe observation per known device.
    fn from_metrics(metrics: Vec<TaskMetrics>) -> Self {
        let mut devices = std::collections::BTreeMap::new();
        for metric in metrics {
            let device_entry = devices
                .entry(String::from(metric.device_id().as_str()))
                .or_insert((Vec::new(), 0_u64));
            let model_name = String::from(metric.model_name());
            if !device_entry.0.contains(&model_name) {
                device_entry.0.push(model_name);
            }
            device_entry.1 = device_entry.1.max(metric.timestamp_epoch_ms());
        }
        Self {
            devices: devices
                .into_iter()
                .map(
                    |(device_id, (supported_models, last_seen))| DeviceResponse {
                        device_id,
                        supports_vad: cfg!(feature = "vad"),
                        supported_models,
                        last_seen,
                    },
                )
                .collect(),
        }
    }
}

/// Serializes one time-series point without inputs, outputs, or identifiers for a task.
#[derive(Serialize)]
struct MetricResponse {
    task_type: &'static str,
    duration_ms: u64,
    latency_ms: u64,
    power_mwh: f32,
    recorded_at: u64,
}

impl From<&TaskMetrics> for MetricResponse {
    fn from(metric: &TaskMetrics) -> Self {
        Self {
            task_type: metric.task_type().as_str(),
            duration_ms: metric.duration_ms(),
            latency_ms: metric.latency_ms(),
            power_mwh: metric.estimated_power_mwh(),
            recorded_at: metric.timestamp_epoch_ms(),
        }
    }
}

/// Serializes the time-series response for one requested device.
#[derive(Serialize)]
struct DeviceTelemetryResponse {
    device_id: String,
    metrics: Vec<MetricResponse>,
}

/// Serializes aggregate values rendered by the dashboard summary cards.
#[derive(Serialize)]
struct TelemetrySummaryResponse {
    total_transcription_time_ms: u64,
    average_latency_ms: u64,
    total_tasks: u64,
    power_estimate_mwh: f32,
}

impl TelemetrySummaryResponse {
    /// Aggregates stored metrics without retaining or serializing task payloads.
    fn from_metrics(metrics: Vec<TaskMetrics>) -> Self {
        let total_tasks = metrics.len() as u64;
        let total_transcription_time_ms = metrics
            .iter()
            .filter(|metric| {
                matches!(
                    metric.task_type(),
                    TaskType::Transcription | TaskType::CloudTranscription
                )
            })
            .map(TaskMetrics::duration_ms)
            .sum();
        let total_latency_ms: u64 = metrics.iter().map(TaskMetrics::latency_ms).sum();
        let power_estimate_mwh = metrics.iter().map(TaskMetrics::estimated_power_mwh).sum();
        Self {
            total_transcription_time_ms,
            average_latency_ms: total_latency_ms
                .checked_div(total_tasks)
                .unwrap_or_default(),
            total_tasks,
            power_estimate_mwh,
        }
    }
}

/// Returns the current UTC epoch in milliseconds, treating pre-epoch systems as zero.
fn current_epoch_ms() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or_default()
}

/// Deserializes the opaque identifiers supplied by the web voice-assignment form.
#[derive(Deserialize)]
struct VoiceAssignmentRequest {
    clip_id: String,
    identity_id: String,
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
