//! Starts the headless Axum daemon with structured JSON logging and injected local adapters.

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use note_taker_core::{
    adapters::observability::{
        LOG_LEVEL_POLL_INTERVAL, LiveLogControl, LogLevelControlFile, LogLevelWatcher,
        LoggingConfiguration, initialize_logging,
    },
    application::transcription_routing::RemoteTranscriptionGateway,
    application::{
        crypto::key_manager::KeyManagerConfig, log_control::LogControl, p2p_service::P2pControl,
    },
    cli_recording::LocalMeetingFiles,
    device_identity::device_id_from_environment,
    domain::models::DeviceId,
    p2p_composition::{P2pConfig, P2pHandle, spawn_p2p_task},
    pipeline_composition::{PipelineConfig, spawn_recording_controller},
    ports::speech_transfer::SpeechLink,
    project_commands::ProjectFiles,
    recording_controller::RecordingController,
    server::api::{ServerAppState, build_router},
    storage_composition::{EncryptedStores, open_encrypted_stores},
    teams_bridge_composition::start_teams_bridge_from_environment,
};

/// Initializes logging, composes headless server ports, and serves the local Axum API.
///
/// Inputs: Reads optional device (`NOTE_TAKER_DEVICE_ID`, else the ID persisted beside the
/// database, see `device_identity`), log, database, bind-address, encryption
/// (`NOTE_TAKER_ENCRYPTION_PASSPHRASE`, `NOTE_TAKER_ENCRYPTION_KEY_DIR`), and meeting-pipeline
/// (see `pipeline_composition`), Teams bridge (`NOTE_TAKER_TEAMS_BRIDGE_*`, see
/// `teams_bridge_composition`), and P2P (`NOTE_TAKER_P2P_*`, `NOTE_TAKER_DEVICE_NAME`, see
/// `p2p_composition`) environment variables.
/// Outputs: A process exit status indicating whether startup or HTTP serving failed.
/// Side effects: Unlocks the embedding encryption key, opens the note repository and the voice
/// store on one local SQLite file, creates logging sinks, composes the meeting pipeline on a
/// blocking thread (a missing engine is logged, not fatal), binds the configured local TCP
/// address, starts the localhost Teams bot bridge when enabled, starts the P2P pairing and sync
/// thread unless disabled (a failure is logged, not fatal), and serves REST/WebSocket
/// requests until SIGINT or SIGTERM. On shutdown it stops accepting requests, closes the bridge
/// (its pump drains buffered media), stops P2P sync, and lets the pipeline worker drain before
/// exiting.
#[tokio::main]
async fn main() -> Result<()> {
    let database_path = env::var("NOTE_TAKER_DATABASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("note_taker.sqlite3"));
    let device_id = device_id_from_environment(&database_path)?;
    let log_directory = env::var("NOTE_TAKER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("logs"));
    let log_filter = env::var("RUST_LOG").unwrap_or_else(|_| String::from("info"));
    let logging_runtime = initialize_logging(
        LoggingConfiguration::new(device_id.clone(), &log_directory).with_filter(&log_filter),
    )?;
    // Shared by `cli log-level` (through the watcher) and the web Settings page (Issue #46).
    let log_control = Arc::new(LiveLogControl::for_runtime(
        logging_runtime,
        &log_filter,
        log_directory,
    ));
    let log_level_watcher = tokio::spawn(apply_log_level_requests(
        Arc::clone(&log_control),
        LogLevelWatcher::start(LogLevelControlFile::for_database(&database_path)),
    ));
    let bind_address =
        env::var("NOTE_TAKER_BIND_ADDRESS").unwrap_or_else(|_| String::from("127.0.0.1:8080"));
    let EncryptedStores {
        note_repository,
        voice_store,
        embedding_cipher,
    } = open_encrypted_stores(
        &database_path,
        device_id.clone(),
        KeyManagerConfig::from_environment(),
    )?;
    // Before P2P starts, so paired devices hear of these deletions at the first exchange (#82).
    if let Err(error) = note_taker_core::application::deletion::remove_leftover_voices(
        &note_repository,
        Some(&LocalMeetingFiles::for_database(&database_path)),
    ) {
        tracing::warn!(error = %error, "leftover unknown voices could not be removed");
    }
    // P2P first: the pipeline routes transcription through its task.
    let p2p_task = start_p2p_task(
        database_path.clone(),
        device_id.clone(),
        Arc::clone(&embedding_cipher),
    )
    .await;
    let recording_controller = compose_recording_pipeline(
        database_path.clone(),
        device_id.clone(),
        Arc::clone(&embedding_cipher),
        p2p_task
            .clone()
            .map(|task| task as Arc<dyn RemoteTranscriptionGateway>),
        p2p_task.clone().map(|task| task as Arc<dyn SpeechLink>),
    )
    .await;
    // The same folder meeting deletions clean; project resources go in `project-<uuid>` there.
    let local_meeting_files = LocalMeetingFiles::for_database(&database_path);
    let project_files = Arc::new(ProjectFiles::new(
        local_meeting_files.attachment_directory().to_path_buf(),
        device_id.clone(),
    ));
    let teams_bridge = start_teams_bridge_from_environment(
        &database_path,
        device_id,
        embedding_cipher,
        recording_controller.as_ref(),
    )
    .await;
    let recording_device = note_repository.device_id().clone();
    let mut application_state = ServerAppState::new(note_repository, voice_store)
        .with_recording_device(
            recording_device,
            note_taker_core::refinement_worker::retained_audio_directory(&database_path),
        )
        .with_log_control(Arc::clone(&log_control) as Arc<dyn LogControl>)
        .with_settings_file(note_taker_core::transcription_settings::settings_file_for(
            &database_path,
        ))
        .with_deletion_hooks(Arc::new(local_meeting_files))
        .with_project_files(project_files)
        .with_llm_services(Arc::new(
            note_taker_core::llm_commands::LlmServices::for_database(&database_path),
        ));
    if let Some(p2p_task) = &p2p_task {
        application_state =
            application_state.with_p2p_control(Arc::clone(p2p_task) as Arc<dyn P2pControl>);
    }
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    tracing::info!(address = %bind_address, "headless Axum server started");
    axum::serve(listener, build_router(application_state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("headless Axum server stopped accepting requests");
    if let Some(teams_bridge) = teams_bridge {
        // Must finish before the controller drops: the pump shares its pipeline worker.
        teams_bridge.shutdown().await;
    }
    if let Some(p2p_task) = p2p_task {
        tokio::task::spawn_blocking(move || p2p_task.shutdown()).await?;
    }
    if let Some(recording_controller) = recording_controller {
        // Dropping the controller stops any recording and waits for queued chunks.
        tokio::task::spawn_blocking(move || drop(recording_controller)).await?;
    }
    tracing::info!("headless daemon shut down");
    // Dropping the task drops the logging runtime last, which flushes the log file.
    log_level_watcher.abort();
    let _ = log_level_watcher.await;
    Ok(())
}

/// Applies `cli log-level` requests to the running daemon (REQUIREMENTS.md 6.1).
///
/// Inputs: The log control (shared with the web Settings routes, so they show the level in
/// effect) and a watcher on `<database>.log-level`.
/// Outputs: Never completes; the daemon aborts it on shutdown.
/// Side effects: Every `LOG_LEVEL_POLL_INTERVAL`, reads the control file (a few bytes, so the
/// read stays on the async runtime) and reloads the tracing filter when a new valid level was
/// requested. Invalid requests are logged as warnings and ignored.
async fn apply_log_level_requests(
    log_control: Arc<LiveLogControl>,
    mut log_level_watcher: LogLevelWatcher,
) {
    let mut poll_interval = tokio::time::interval(LOG_LEVEL_POLL_INTERVAL);
    loop {
        poll_interval.tick().await;
        match log_level_watcher.poll() {
            Ok(Some(filter_directive)) => match log_control.set_level(&filter_directive) {
                Ok(level) => tracing::warn!(level = %level, "log level changed"),
                Err(error) => {
                    tracing::warn!(error = %format!("{error:#}"), "unable to change the log level");
                }
            },
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "ignored an invalid log level request");
            }
        }
    }
}

/// Composes the meeting pipeline without blocking the async runtime.
///
/// Inputs: The database path, the acquisition device ID, and the verified embedding cipher.
/// Outputs: The idle recording controller, or `None` when no transcription engine is
/// configured (logged as a warning; the daemon keeps serving notes).
/// Side effects: Spawns the pipeline worker on a blocking thread; it may load a local model and
/// opens its own SQLite connections. Logs which optional pipeline stages are available.
async fn compose_recording_pipeline(
    database_path: PathBuf,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn note_taker_core::ports::crypto::EmbeddingCipher>,
    remote_gateway: Option<Arc<dyn RemoteTranscriptionGateway>>,
    speech_link: Option<Arc<dyn SpeechLink>>,
) -> Option<RecordingController> {
    let composition = tokio::task::spawn_blocking(move || {
        spawn_recording_controller(
            &database_path,
            device_id,
            embedding_cipher,
            PipelineConfig::from_environment()?,
            remote_gateway,
            speech_link,
        )
    })
    .await;
    match composition {
        Ok(Ok(recording_controller)) => {
            let availability = recording_controller.recording_manager().availability();
            tracing::info!(
                diarization_available = availability.diarization,
                voice_matching_available = availability.voice_matching,
                "meeting pipeline ready"
            );
            Some(recording_controller)
        }
        Ok(Err(error)) => {
            tracing::warn!(error = %format!("{error:#}"), "meeting pipeline unavailable");
            None
        }
        Err(join_error) => {
            tracing::error!(error = %join_error, "meeting pipeline composition panicked");
            None
        }
    }
}

/// Starts P2P pairing and sync without blocking the async runtime.
///
/// Inputs: The database path, this device's ID, and the verified embedding cipher.
/// Outputs: The task's handle, or `None` when P2P is disabled or could not start (logged; the
/// daemon keeps serving local notes).
/// Side effects: Reads the P2P environment and, when enabled, spawns the P2P thread, which opens
/// its own SQLite connections and binds the listen address.
async fn start_p2p_task(
    database_path: PathBuf,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn note_taker_core::ports::crypto::EmbeddingCipher>,
) -> Option<Arc<P2pHandle>> {
    let mut config = match P2pConfig::from_environment() {
        Ok(Some(config)) => config,
        Ok(None) => {
            tracing::info!("P2P sync disabled");
            return None;
        }
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "P2P sync unavailable");
            return None;
        }
    };
    // The manifest advertises what the pipeline can run; an invalid pipeline setting is
    // reported by the pipeline itself, so it advertises no models here.
    let mut hosted_pipeline = None;
    if let Ok(pipeline_config) = PipelineConfig::from_environment() {
        config.capabilities.transcription_models =
            pipeline_config.advertised_transcription_models();
        config.capabilities.gpu_vram_megabytes =
            note_taker_core::pipeline_composition::local_gpu_megabytes();
        if !config.capabilities.transcription_models.is_empty() {
            hosted_pipeline = Some(pipeline_config);
        }
    }
    let started = tokio::task::spawn_blocking(move || {
        spawn_p2p_task(
            &database_path,
            device_id,
            embedding_cipher,
            config,
            hosted_pipeline,
        )
    })
    .await;
    match started {
        Ok(Ok(p2p_task)) => Some(Arc::new(p2p_task)),
        Ok(Err(error)) => {
            tracing::warn!(error = %format!("{error:#}"), "P2P sync unavailable");
            None
        }
        Err(join_error) => {
            tracing::error!(error = %join_error, "P2P sync start-up panicked");
            None
        }
    }
}

/// Resolves when the process receives SIGINT (Ctrl+C) or, on Unix, SIGTERM.
///
/// Inputs: None.
/// Outputs: Completes once a shutdown signal arrives; never completes if no handler installs.
/// Side effects: Installs signal handlers for the rest of the process.
async fn shutdown_signal() {
    let interrupt = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate_signal) => {
                terminate_signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => tracing::info!("SIGINT received; shutting down"),
        () = terminate => tracing::info!("SIGTERM received; shutting down"),
    }
}
