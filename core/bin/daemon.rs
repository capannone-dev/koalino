//! Starts the headless Axum daemon with structured JSON logging and injected local adapters.

use std::{env, path::PathBuf};

use anyhow::Result;
use note_taker_core::{
    adapters::observability::{LoggingConfiguration, initialize_logging},
    application::crypto::key_manager::KeyManagerConfig,
    domain::models::DeviceId,
    server::api::{ServerAppState, build_router},
    storage_composition::{EncryptedStores, open_encrypted_stores},
};

/// Initializes logging, composes headless server ports, and serves the local Axum API.
///
/// Inputs: Reads optional device, log, database, bind-address, and encryption
/// (`NOTE_TAKER_ENCRYPTION_PASSPHRASE`, `NOTE_TAKER_ENCRYPTION_KEY_DIR`) environment variables.
/// Outputs: A process exit status indicating whether startup or HTTP serving failed.
/// Side effects: Unlocks the embedding encryption key, opens the note repository and the voice
/// store on one local SQLite file, creates logging sinks, binds the configured local TCP address,
/// and serves REST/WebSocket requests until the process is stopped.
#[tokio::main]
async fn main() -> Result<()> {
    let device_id = DeviceId::new(
        env::var("NOTE_TAKER_DEVICE_ID").unwrap_or_else(|_| String::from("unconfigured-device")),
    );
    let log_directory = env::var("NOTE_TAKER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("logs"));
    let log_filter = env::var("RUST_LOG").unwrap_or_else(|_| String::from("info"));
    let _logging_runtime = initialize_logging(
        LoggingConfiguration::new(device_id, log_directory).with_filter(log_filter),
    )?;
    let database_path = env::var("NOTE_TAKER_DATABASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("note_taker.sqlite3"));
    let bind_address =
        env::var("NOTE_TAKER_BIND_ADDRESS").unwrap_or_else(|_| String::from("127.0.0.1:8080"));
    let EncryptedStores {
        note_repository,
        voice_store,
        ..
    } = open_encrypted_stores(
        &database_path,
        DeviceId::new("headless-server"),
        KeyManagerConfig::from_environment(),
    )?;
    let application_state = ServerAppState::new(note_repository, voice_store);
    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    tracing::info!(address = %bind_address, "headless Axum server started");
    axum::serve(listener, build_router(application_state)).await?;
    Ok(())
}
