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
            ClusterReviewError, NamedClusterAssignment, NamedGroupAssignment, SavedPersonSummary,
            UnknownClusterSummary, VoiceLibraryCluster, assign_cluster_to_named_identity,
            assign_clusters_to_named_identity, find_cluster_review_clip, list_unknown_clusters,
            load_voice_library,
        },
        combined_meetings::{
            CombiningView, MeetingListing, RecordingDevice, combining_view, name_recording_devices,
            split_recordings,
        },
        deletion::{self, DeletionError, MeetingDeletionHooks},
        export_manager::{ExportManager, ExportWriter},
        log_control::LogControl,
        meeting_crdt::MeetingCrdtService,
        meeting_rename::{MeetingRenameError, rename_meeting},
        p2p_service::P2pControl,
        unknown_speaker_clusterer::ClusteringError,
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::{ExportError, StorageError},
        models::{
            AggregatedStats, Attachment, DeviceId, ExportFormat, ExportOptions, MeetingId,
            MeetingMetadata, MeetingTitle, TaskMetrics, TaskType, UnassignedVoiceClip,
        },
    },
    ports::{
        projects::ProjectRepository,
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
        voice::VoiceVectorDB,
    },
};

/// Combines the repository capabilities required by the headless server boundary, including
/// projects (Issue #85).
pub trait ServerRepository:
    NoteRepository
    + AttachmentRepository
    + MetricsRepository
    + ProjectRepository
    + crate::ports::chat::ChatHistoryRepository
    + Send
{
}

impl<T> ServerRepository for T where
    T: NoteRepository
        + AttachmentRepository
        + MetricsRepository
        + ProjectRepository
        + crate::ports::chat::ChatHistoryRepository
        + Send
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
    /// The process's log level and log directory, for the web Settings page (Issue #46).
    log_control: Option<Arc<dyn LogControl>>,
    /// Checks for a recording and removes files on deletion; `None` skips both.
    deletion_hooks: Option<Arc<dyn MeetingDeletionHooks>>,
    /// Where project resources live (Issue #85); `None` makes the file routes answer 503.
    project_files: Option<Arc<crate::project_commands::ProjectFiles>>,
    /// This device and the speech it holds, for meetings' combining status (#88); `None` says
    /// nothing is held here.
    recording_device: Option<Arc<RecordingDeviceState>>,
    /// The transcription settings file holding the glossary; `None` makes its routes answer 503.
    settings_file: Option<Arc<std::path::PathBuf>>,
    /// Language model connections and summaries (#95); `None` makes their routes answer 503.
    llm: Option<Arc<crate::llm_commands::LlmServices>>,
    /// The notes folder (#98); `None` makes its routes answer 503.
    notes_folder: Option<Arc<crate::notes_folder_commands::NotesFolderKeeper>>,
}

/// This device as the meetings name it, and the speech it holds for the after-meeting pass.
struct RecordingDeviceState {
    this_device: DeviceId,
    retained_speech: crate::adapters::audio::retained_audio_store::RetainedSpeechIndex,
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
            log_control: None,
            deletion_hooks: None,
            project_files: None,
            recording_device: None,
            settings_file: None,
            llm: None,
            notes_folder: None,
        }
    }

    /// Attaches this device's ID and the folder of the speech it holds, which meetings'
    /// combining status reads (#88).
    ///
    /// Side effects: None; without it no speech counts as held here.
    pub fn with_recording_device(
        mut self,
        this_device: DeviceId,
        retained_audio_directory: std::path::PathBuf,
    ) -> Self {
        self.recording_device = Some(Arc::new(RecordingDeviceState {
            this_device,
            retained_speech: crate::adapters::audio::retained_audio_store::RetainedSpeechIndex::new(
                retained_audio_directory,
            ),
        }));
        self
    }

    /// Works out a listed meeting's combining status as this device sees it (#88).
    ///
    /// Side effects: Reads the retained speech folder's metadata.
    pub(super) fn combining_of(
        &self,
        meeting_id: &MeetingId,
        recordings: &[crate::ports::storage::MeetingRecording],
        p2p_status: Option<&crate::application::p2p_service::P2pStatus>,
    ) -> CombiningView {
        let now = std::time::SystemTime::now();
        let unknown_device = DeviceId::new("");
        let state = self.recording_device.as_deref();
        let held = |meeting: &MeetingId| {
            state.and_then(|state| state.retained_speech.held_speech(meeting, now))
        };
        combining_view(
            meeting_id,
            recordings,
            &held,
            state.map_or(&unknown_device, |state| &state.this_device),
            (
                chrono::Utc::now(),
                crate::transcription_settings::keep_for_in(self.settings_file().as_deref()),
            ),
            p2p_status,
        )
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

    /// Attaches the transcription settings file, which the glossary routes read and write.
    ///
    /// Side effects: None; without it those routes answer 503.
    pub fn with_settings_file(mut self, settings_file: std::path::PathBuf) -> Self {
        self.settings_file = Some(Arc::new(settings_file));
        self
    }

    /// Reports whether the meeting is being recorded now, as the deletion hooks see it; false
    /// without hooks.
    pub(super) fn is_recording(&self, meeting_id: &MeetingId) -> bool {
        self.deletion_hooks
            .as_ref()
            .is_some_and(|hooks| hooks.is_recording(meeting_id))
    }

    /// This device and the speech it holds for the after-meeting pass (#101).
    ///
    /// Outputs: This device's ID and the meeting's held speech; `None` without
    /// `with_recording_device`. Side effects: Reads the retained speech folder's metadata.
    pub(super) fn held_speech(
        &self,
        meeting_id: &MeetingId,
    ) -> Option<(
        DeviceId,
        Option<crate::application::combined_meetings::HeldSpeech>,
    )> {
        self.recording_device.as_deref().map(|state| {
            (
                state.this_device.clone(),
                state
                    .retained_speech
                    .held_speech(meeting_id, std::time::SystemTime::now()),
            )
        })
    }

    /// The attached settings file, if any.
    pub(super) fn settings_file(&self) -> Option<std::path::PathBuf> {
        self.settings_file.as_deref().cloned()
    }

    /// Attaches the process's log control, which the Settings routes use.
    ///
    /// Side effects: None; without it those routes answer 503.
    pub fn with_log_control(mut self, log_control: Arc<dyn LogControl>) -> Self {
        self.log_control = Some(log_control);
        self
    }

    /// Attaches what deleting a meeting needs from outside the database.
    ///
    /// Side effects: None; without it deletions skip the recording check and leave files.
    pub fn with_deletion_hooks(mut self, deletion_hooks: Arc<dyn MeetingDeletionHooks>) -> Self {
        self.deletion_hooks = Some(deletion_hooks);
        self
    }

    /// Attaches the project resource folder, which the `/api/projects` file routes use.
    ///
    /// Side effects: None; without it those routes answer 503.
    pub fn with_project_files(
        mut self,
        project_files: Arc<crate::project_commands::ProjectFiles>,
    ) -> Self {
        self.project_files = Some(project_files);
        self
    }

    /// The attached project files, if any.
    pub(super) fn project_files(&self) -> Option<Arc<crate::project_commands::ProjectFiles>> {
        self.project_files.clone()
    }

    /// Attaches the language model services, which the summary and AI settings routes use.
    ///
    /// Side effects: None; without them those routes answer 503.
    pub fn with_llm_services(mut self, llm: Arc<crate::llm_commands::LlmServices>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Attaches the notes folder keeper, which the notes folder routes and rounds use.
    ///
    /// Side effects: None; without it those routes answer 503.
    pub fn with_notes_folder(
        mut self,
        keeper: Arc<crate::notes_folder_commands::NotesFolderKeeper>,
    ) -> Self {
        self.notes_folder = Some(keeper);
        self
    }

    /// The attached notes folder keeper, if any.
    pub(super) fn notes_folder(
        &self,
    ) -> Option<Arc<crate::notes_folder_commands::NotesFolderKeeper>> {
        self.notes_folder.clone()
    }

    /// The attached language model services, if any.
    pub(super) fn llm(&self) -> Option<Arc<crate::llm_commands::LlmServices>> {
        self.llm.clone()
    }

    /// The shared voice database, for routes that take the lock themselves.
    pub(super) fn voice_database(&self) -> Arc<Mutex<Box<dyn VoiceVectorDB + Send>>> {
        Arc::clone(&self.voice_database)
    }

    /// The shared repositories, for routes that take the lock themselves.
    pub(super) fn repositories(&self) -> Arc<Mutex<Box<dyn ServerRepository>>> {
        Arc::clone(&self.repositories)
    }

    /// The attached log control, if any.
    pub(super) fn log_control(&self) -> Option<Arc<dyn LogControl>> {
        self.log_control.clone()
    }

    /// The attached P2P task, if any.
    pub(super) fn p2p_control(&self) -> Option<Arc<dyn P2pControl>> {
        self.p2p_control.clone()
    }

    /// Tells the P2P task, if any, that `meeting_id` was just written, so connected devices get it.
    ///
    /// Side effects: Queues a notification without waiting; nothing without P2P.
    pub(super) fn notify_local_change(&self, meeting_id: &MeetingId) {
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
            get(get_meeting)
                .patch(rename_meeting_title)
                .delete(delete_meeting_route),
        )
        .route("/api/meetings/{meeting_id}/export", post(export_meeting))
        .route(
            "/api/meetings/{meeting_id}/split",
            post(split_meeting_route),
        )
        .route(
            "/api/meetings/{meeting_id}/unknown-clusters",
            get(get_unknown_clusters),
        )
        .route(
            "/api/clusters/{cluster_id}",
            axum::routing::delete(delete_cluster_route),
        )
        .route(
            "/api/identities/{identity_id}",
            axum::routing::delete(delete_identity_route),
        )
        .route("/api/clusters/{cluster_id}/assign", post(assign_cluster))
        .route("/api/voice-groups/assign", post(assign_voice_group))
        .route(
            "/api/clusters/{cluster_id}/review-clip",
            get(get_cluster_review_clip),
        )
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
        .merge(super::project_routes::routes())
        .merge(super::log_routes::routes())
        .merge(super::glossary_routes::routes())
        .merge(super::retention_routes::routes())
        .merge(super::meeting_voices_routes::routes())
        .merge(super::llm_routes::routes())
        .merge(super::edit_routes::routes())
        .merge(super::search_routes::routes())
        .merge(super::notes_folder_routes::routes())
        .merge(super::chat_routes::routes())
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
                    axum::http::Method::PUT,
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
    // A meeting in no project, or in one not synced here yet, has a null `project_id`.
    let meeting_projects: std::collections::HashMap<MeetingId, String> = repositories
        .meeting_projects()
        .map_err(storage_error_status)?
        .into_iter()
        .map(|(meeting_id, project_id)| (meeting_id, project_id.to_string()))
        .collect();
    let recordings = repositories
        .meeting_recordings()
        .map_err(storage_error_status)?;
    let listing = MeetingListing::of(&recordings);
    let p2p_status = application_state
        .p2p_control()
        .map(|p2p_control| p2p_control.snapshot().status);
    let mut meeting_summaries = Vec::with_capacity(meeting_ids.len());
    for meeting_id in meeting_ids {
        if !listing.is_listed(&meeting_id) {
            continue;
        }
        if let Some(meeting_metadata) = repositories
            .load_meeting_metadata(&meeting_id)
            .map_err(storage_error_status)?
        {
            let mut summary = MeetingSummary::from(&meeting_metadata);
            summary.project_id = meeting_projects.get(&meeting_id).cloned();
            summary.recorded_on =
                name_recording_devices(&listing.recorded_on(&meeting_id), p2p_status.as_ref());
            summary.combined_recordings = meeting_id_strings(&listing.folded_into(&meeting_id));
            summary.combining =
                Some(application_state.combining_of(&meeting_id, &recordings, p2p_status.as_ref()));
            meeting_summaries.push(summary);
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

/// Returns one meeting's title, readable transcript (recorded segments, or imported text), and
/// linked metadata-only attachments.
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
    let transcript =
        crate::application::meeting_transcript::readable_transcript(&**repositories, &meeting_id)
            .map_err(storage_error_status)?;
    let mut stored_segments = repositories
        .load_transcript_segments(&meeting_id)
        .map_err(storage_error_status)?;
    stored_segments.sort_by_key(|segment| (segment.start_time_ms(), segment.end_time_ms()));
    let speaker_names: std::collections::HashMap<uuid::Uuid, String> = application_state
        .voice_database
        .lock()
        .await
        .list_speaker_identities()
        .map_err(storage_error_status)?
        .into_iter()
        .map(|identity| (identity.identity_id(), identity.display_name().to_owned()))
        .collect();
    let segments = stored_segments
        .iter()
        .map(|segment| SegmentView {
            start_time_ms: segment.start_time_ms(),
            end_time_ms: segment.end_time_ms(),
            speaker: crate::application::export_manager::segment_speaker_label(
                segment,
                |identity_id| speaker_names.get(&identity_id).map(String::as_str),
            )
            .to_owned(),
            text: segment.text().to_owned(),
            overlapping_speech: segment.confidence().overlapping_speech,
            uncertain_speaker: segment.confidence().uncertain_speaker,
            uncertain_words: segment.confidence().uncertain_words.clone(),
        })
        .collect();
    let attachments = repositories
        .get_attachments_for_meeting(&meeting_id)
        .map_err(storage_error_status)?;
    let recordings = repositories
        .meeting_recordings()
        .map_err(storage_error_status)?;
    let listing = MeetingListing::of(&recordings);
    let p2p_status = application_state
        .p2p_control()
        .map(|p2p_control| p2p_control.snapshot().status);
    Ok(Json(MeetingDetail {
        meeting_id: String::from(meeting_id.as_str()),
        title: meeting_metadata.display_title(),
        created_at: meeting_metadata.created_at().to_rfc3339(),
        transcription_languages: transcription_language_codes(&meeting_metadata),
        recorded_on: name_recording_devices(&listing.recorded_on(&meeting_id), p2p_status.as_ref()),
        combined_recordings: meeting_id_strings(&listing.folded_into(&meeting_id)),
        combining: application_state.combining_of(&meeting_id, &recordings, p2p_status.as_ref()),
        transcript,
        segments,
        attachments: attachments.iter().map(AttachmentSummary::from).collect(),
    }))
}

/// Lists meeting IDs as strings. Side effects: None.
fn meeting_id_strings(meeting_ids: &[MeetingId]) -> Vec<String> {
    meeting_ids
        .iter()
        .map(|meeting_id| meeting_id.as_str().to_owned())
        .collect()
}

/// Splits the recordings combined into a meeting apart again, for good (#88).
///
/// Inputs: The listed meeting's ID, or one combined recording's ID (only it is split).
/// Outputs: `200` with `{"split": [<meeting IDs listed on their own again>]}` (empty when there
/// was nothing to split), `404` for an unknown meeting, or a storage error status.
/// Side effects: Writes each split recording's document and tells paired devices.
async fn split_meeting_route(
    Path(meeting_identifier): Path<String>,
    State(application_state): State<ServerAppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!(endpoint = "POST /api/meetings/:meeting_id/split", meeting_id = %meeting_identifier, "headless API request received");
    let meeting_id = MeetingId::new(meeting_identifier);
    let repositories = application_state.repositories.lock().await;
    repositories
        .load_meeting_metadata(&meeting_id)
        .map_err(storage_error_status)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let split = split_recordings(&**repositories, &meeting_id).map_err(storage_error_status)?;
    drop(repositories);
    for recording in &split {
        application_state.notify_local_change(recording);
    }
    Ok(Json(
        serde_json::json!({ "split": meeting_id_strings(&split) }),
    ))
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
    let mut export_writer = ExportWriter::new(default_exporters(), &request_directory);
    // Staged attachments are opened with the database key (#99).
    if let Some(project_files) = &application_state.project_files {
        export_writer =
            export_writer.with_attachment_files(Arc::new(project_files.sealed_files().clone()));
    }
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

/// Maps a deletion's outcome to its response.
///
/// Outputs: `204` when done; `404` unknown; `409` being recorded; `500` storage. Errors are
/// `{"error": "…"}`. Side effects: None.
fn deletion_response(deletion_result: Result<(), DeletionError>) -> Response {
    let status_code = match &deletion_result {
        Ok(()) => return StatusCode::NO_CONTENT.into_response(),
        Err(DeletionError::NotFound { .. }) => StatusCode::NOT_FOUND,
        Err(DeletionError::Recording { .. }) => StatusCode::CONFLICT,
        Err(DeletionError::Storage(_)) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let message = deletion_result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    (status_code, Json(ExportErrorBody { error: message })).into_response()
}

/// Deletes a meeting with its transcript, attachments and unknown voices.
///
/// Inputs: The meeting ID path segment.
/// Outputs: As `deletion_response`.
/// Side effects: Irreversibly deletes its rows and private files. The P2P task spreads the deletion
/// to paired devices within about a second.
async fn delete_meeting_route(
    Path(meeting_identifier): Path<String>,
    State(application_state): State<ServerAppState>,
) -> Response {
    tracing::info!(endpoint = "DELETE /api/meetings/:meeting_id", meeting_id = %meeting_identifier, "headless API request received");
    let repositories = application_state.repositories.lock().await;
    deletion_response(deletion::delete_meeting(
        &**repositories,
        application_state.deletion_hooks.as_deref(),
        &MeetingId::new(meeting_identifier),
    ))
}

/// Deletes an unknown voice without naming it. Its transcript stays.
///
/// Inputs: The cluster UUID path segment; a malformed one gets axum's `400`.
/// Outputs: As `deletion_response`.
/// Side effects: Irreversibly deletes its embeddings and review audio, and spreads as above.
async fn delete_cluster_route(
    Path(cluster_id): Path<uuid::Uuid>,
    State(application_state): State<ServerAppState>,
) -> Response {
    tracing::info!(endpoint = "DELETE /api/clusters/:cluster_id", cluster_id = %cluster_id, "headless API request received");
    let repositories = application_state.repositories.lock().await;
    deletion_response(deletion::delete_unknown_voice(
        &**repositories,
        application_state.deletion_hooks.as_deref(),
        cluster_id,
    ))
}

/// Deletes a saved person's name and voice samples. Transcripts show an unknown speaker afterwards.
///
/// Inputs: The identity UUID path segment; a malformed one gets axum's `400`.
/// Outputs: `204`, also for an unknown person, or `500` with `{"error": "…"}`.
/// Side effects: Irreversibly deletes biometric data, and spreads as above.
async fn delete_identity_route(
    Path(identity_id): Path<uuid::Uuid>,
    State(application_state): State<ServerAppState>,
) -> Response {
    tracing::info!(endpoint = "DELETE /api/identities/:identity_id", identity_id = %identity_id, "headless API request received");
    let voice_database = application_state.voice_database.lock().await;
    deletion_response(
        voice_database
            .delete_identity(identity_id)
            .map_err(DeletionError::from),
    )
}

/// Returns speaker labels and metadata-only unresolved review items for the local gallery.
async fn get_voice_gallery(
    State(application_state): State<ServerAppState>,
) -> Result<Json<VoiceGalleryResponse>, StatusCode> {
    tracing::info!(
        endpoint = "/api/voice-gallery",
        "headless API request received"
    );
    let repositories = application_state.repositories.lock().await;
    let voice_database = application_state.voice_database.lock().await;
    let library = load_voice_library(&**repositories, &**voice_database)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    drop(voice_database);
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
        identities: library
            .saved_people()
            .iter()
            .map(IdentitySummary::from)
            .collect(),
        unknown_clusters: library
            .unknown_clusters()
            .iter()
            .map(LibraryClusterResponse::from)
            .collect(),
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

/// Names several unknown voices as one person, as when the owner names a voice group (#82).
///
/// Inputs: A `{"cluster_ids": ["<uuid>", …], "identity_name": "…"}` body.
/// Outputs: `200` with a `GroupAssignmentResponse`; `400` for a blank name or no clusters; Axum's
/// `422` for a malformed body or ID; `404`/`409` when every cluster is gone or already named; `500` for storage
/// failures. Clusters gone or named meanwhile are skipped and counted.
/// Side effects: Under the repository and voice locks, does what `assign_cluster` does for each
/// cluster, to one identity.
async fn assign_voice_group(
    State(application_state): State<ServerAppState>,
    Json(group_request): Json<GroupAssignmentRequest>,
) -> Response {
    tracing::info!(
        endpoint = "/api/voice-groups/assign",
        cluster_count = group_request.cluster_ids.len(),
        "headless API request received"
    );
    let repositories = application_state.repositories.lock().await;
    let voice_database = application_state.voice_database.lock().await;
    match assign_clusters_to_named_identity(
        &**repositories,
        &**voice_database,
        &group_request.cluster_ids,
        &group_request.identity_name,
    ) {
        Ok(assignment) => Json(GroupAssignmentResponse::from(&assignment)).into_response(),
        Err(error) => cluster_review_error_response(&error),
    }
}

/// Serves a cluster's review clip so the dashboard can play it (Issue #63).
///
/// Inputs: `cluster_id` path segment (a UUID; anything else is rejected with `400` by Axum).
/// Outputs: `200` with the WAV bytes and `Cache-Control: no-store`; `404` when the cluster has no
/// clip; `500` for storage failures.
/// Side effects: Reads and decrypts the clip; nothing is written, and its bytes are never logged.
async fn get_cluster_review_clip(
    Path(cluster_id): Path<uuid::Uuid>,
    State(application_state): State<ServerAppState>,
) -> Response {
    tracing::info!(endpoint = "/api/clusters/:cluster_id/review-clip", cluster_id = %cluster_id, "headless API request received");
    let repositories = application_state.repositories.lock().await;
    match find_cluster_review_clip(&**repositories, cluster_id) {
        Ok(Some(clip)) => (
            [
                (axum::http::header::CONTENT_TYPE, "audio/wav"),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            clip.audio_bytes().to_vec(),
        )
            .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => cluster_review_error_response(&error),
    }
}

/// Maps cluster-review failures to stable HTTP statuses with a `{"error": "…"}` body.
fn cluster_review_error_response(error: &ClusterReviewError) -> Response {
    let status_code = match error {
        ClusterReviewError::BlankIdentityName | ClusterReviewError::NoClustersSelected => {
            StatusCode::BAD_REQUEST
        }
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

/// Request body for `POST /api/voice-groups/assign`.
#[derive(Deserialize)]
struct GroupAssignmentRequest {
    /// The voices to name, in order; a malformed UUID rejects the request.
    cluster_ids: Vec<uuid::Uuid>,
    /// Person's name; a known identity with this name is reused, otherwise one is created.
    identity_name: String,
}

/// Summarizes a voice group named as one person.
#[derive(Serialize)]
struct GroupAssignmentResponse {
    identity_id: String,
    identity_name: String,
    /// `true` when this naming created the identity.
    identity_created: bool,
    assigned_cluster_ids: Vec<String>,
    /// Voices skipped because they were already named or gone.
    skipped_cluster_count: usize,
    variants_stored: usize,
    segments_updated: usize,
}

impl From<&NamedGroupAssignment> for GroupAssignmentResponse {
    fn from(assignment: &NamedGroupAssignment) -> Self {
        Self {
            identity_id: assignment.identity_id().to_string(),
            identity_name: String::from(assignment.identity_name()),
            identity_created: assignment.identity_created(),
            assigned_cluster_ids: assignment
                .assigned_cluster_ids()
                .iter()
                .map(ToString::to_string)
                .collect(),
            skipped_cluster_count: assignment.skipped_cluster_count(),
            variants_stored: assignment.variants_stored(),
            segments_updated: assignment.segments_updated(),
        }
    }
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
    /// The project the meeting is in (Issue #85); null outside any project. Only the list sets it.
    project_id: Option<String>,
    /// Speech-to-text languages (`auto` or Whisper codes) in first-use order; empty when none
    /// was recorded.
    transcription_languages: Vec<&'static str>,
    /// The devices that recorded the meeting (#88); only the list sets it.
    recorded_on: Vec<RecordingDevice>,
    /// Other devices' recordings combined into this meeting, which Split lists again; only the
    /// list sets it.
    combined_recordings: Vec<String>,
    /// How far combining the recordings got, as this device sees it (#88); only the list sets it.
    #[serde(skip_serializing_if = "Option::is_none")]
    combining: Option<CombiningView>,
}
impl From<&MeetingMetadata> for MeetingSummary {
    fn from(meeting_metadata: &MeetingMetadata) -> Self {
        Self {
            meeting_id: String::from(meeting_metadata.meeting_id().as_str()),
            title: meeting_metadata.display_title(),
            created_at: meeting_metadata.created_at().to_rfc3339(),
            project_id: None,
            transcription_languages: transcription_language_codes(meeting_metadata),
            recorded_on: Vec::new(),
            combined_recordings: Vec::new(),
            combining: None,
        }
    }
}

/// Lists a meeting's speech-to-text languages as their text codes. Side effects: None.
fn transcription_language_codes(meeting_metadata: &MeetingMetadata) -> Vec<&'static str> {
    meeting_metadata
        .transcription_languages()
        .iter()
        .map(|language| language.as_str())
        .collect()
}

/// Serializes a complete meeting response without CRDT data or attachment content.
#[derive(Serialize)]
struct MeetingDetail {
    meeting_id: String,
    /// The stored title, or the dated fallback for an untitled meeting.
    title: String,
    /// RFC 3339 UTC creation time.
    created_at: String,
    /// Speech-to-text languages in first-use order; empty when none was recorded.
    transcription_languages: Vec<&'static str>,
    /// The devices that recorded the meeting (#88).
    recorded_on: Vec<RecordingDevice>,
    /// Other devices' recordings combined into this meeting, which Split lists again.
    combined_recordings: Vec<String>,
    /// How far combining the recordings got, as this device sees it (#88).
    combining: CombiningView,
    transcript: Option<String>,
    /// The recorded segments in time order with their speaker and confidence marks (#90, #91);
    /// empty for a meeting imported as text.
    segments: Vec<SegmentView>,
    attachments: Vec<AttachmentSummary>,
}

/// Serializes one transcript segment for the meeting page.
#[derive(Serialize)]
struct SegmentView {
    start_time_ms: u64,
    end_time_ms: u64,
    /// The person's name, `Unknown speaker`, or `Unattributed`.
    speaker: String,
    text: String,
    /// The segment lies largely in overlapping speech.
    overlapping_speech: bool,
    /// The speaker is a guess.
    uncertain_speaker: bool,
    /// Positions of the uncertain words among the text's whitespace-separated words.
    uncertain_words: Vec<u16>,
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
    /// Unresolved clusters of every meeting, without embeddings (Issue #63).
    unknown_clusters: Vec<LibraryClusterResponse>,
    unassigned_clips: Vec<UnassignedClipSummary>,
}
#[derive(Serialize)]
struct IdentitySummary {
    identity_id: String,
    display_name: String,
    /// How many voice variants are stored; the variants themselves are never serialized.
    voice_variant_count: usize,
}
impl From<&SavedPersonSummary> for IdentitySummary {
    fn from(person: &SavedPersonSummary) -> Self {
        Self {
            identity_id: person.identity_id().to_string(),
            display_name: String::from(person.display_name()),
            voice_variant_count: person.voice_variant_count(),
        }
    }
}
/// One unresolved cluster with what the gallery shows beside it.
#[derive(Serialize)]
struct LibraryClusterResponse {
    #[serde(flatten)]
    cluster: UnknownClusterResponse,
    /// The meeting's title; `null` for an untitled meeting.
    meeting_title: Option<String>,
    /// Whether `GET /api/clusters/{id}/review-clip` has audio to serve.
    has_review_clip: bool,
    /// The voice group it belongs to, shared by voices that sound like one person (#82).
    voice_group_id: String,
}
impl From<&VoiceLibraryCluster> for LibraryClusterResponse {
    fn from(library_cluster: &VoiceLibraryCluster) -> Self {
        Self {
            cluster: UnknownClusterResponse::from(library_cluster.summary()),
            meeting_title: library_cluster.meeting_title().map(String::from),
            has_review_clip: library_cluster.has_review_clip(),
            voice_group_id: library_cluster.voice_group_id().to_string(),
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
