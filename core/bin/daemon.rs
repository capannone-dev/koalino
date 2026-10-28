//! Starts the headless Axum daemon with structured JSON logging and injected local adapters.

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use note_taker_core::{
    adapters::observability::{
        LOG_LEVEL_POLL_INTERVAL, LogLevelControlFile, LogLevelWatcher, LoggingConfiguration,
        LoggingRuntime, initialize_logging,
    },
    application::crypto::key_manager::KeyManagerConfig,
    device_identity::device_id_from_environment,
    domain::models::DeviceId,
    pipeline_composition::{PipelineConfig, spawn_recording_controller},
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
/// (see `pipeline_composition`), and Teams bridge (`NOTE_TAKER_TEAMS_BRIDGE_*`, see
/// `teams_bridge_composition`) environment variables.
/// Outputs: A process exit status indicating whether startup or HTTP serving failed.
/// Side effects: Unlocks the embedding encryption key, opens the note repository and the voice
/// store on one local SQLite file, creates logging sinks, composes the meeting pipeline on a
/// blocking thread (a missing engine is logged, not fatal), binds the configured local TCP
/// address, starts the localhost Teams bot bridge when enabled, and serves REST/WebSocket
/// requests until SIGINT or SIGTERM. On shutdown it stops accepting requests, closes the bridge
/// (its pump drains buffered media), and lets the pipeline worker drain before exiting.
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
        LoggingConfiguration::new(device_id.clone(), log_directory).with_filter(log_filter),
    )?;
    let log_level_watcher = tokio::spawn(apply_log_level_requests(
        logging_runtime,
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
    let recording_controller = compose_recording_pipeline(
        database_path.clone(),
        device_id.clone(),
        Arc::clone(&embedding_cipher),
    )
    .await;
    let teams_bridge = start_teams_bridge_from_environment(
        &database_path,
        device_id,
        embedding_cipher,
        recording_controller.as_ref(),
    )
    .await;
    let application_state = ServerAppState::new(note_repository, voice_store);
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
/// Inputs: The logging runtime (owned here so it lives as long as the daemon) and a watcher on
/// `<database>.log-level`.
/// Outputs: Never completes; the daemon aborts it on shutdown.
/// Side effects: Every `LOG_LEVEL_POLL_INTERVAL`, reads the control file (a few bytes, so the
/// read stays on the async runtime) and reloads the tracing filter when a new valid level was
/// requested. Invalid requests are logged as warnings and ignored.
async fn apply_log_level_requests(
    logging_runtime: LoggingRuntime,
    mut log_level_watcher: LogLevelWatcher,
) {
    let mut poll_interval = tokio::time::interval(LOG_LEVEL_POLL_INTERVAL);
    loop {
        poll_interval.tick().await;
        match log_level_watcher.poll() {
            Ok(Some(filter_directive)) => match logging_runtime.set_filter(&filter_directive) {
                Ok(()) => tracing::warn!(level = %filter_directive, "log level changed"),
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
) -> Option<RecordingController> {
    let composition = tokio::task::spawn_blocking(move || {
        spawn_recording_controller(
            &database_path,
            device_id,
            embedding_cipher,
            PipelineConfig::from_environment()?,
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
