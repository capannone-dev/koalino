//! Starts the interactive command-line composition root with quiet logging and injected services.

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::Parser;
use note_taker_core::{
    adapters::{
        export::{default_export_directory, default_exporters},
        observability::{LoggingConfiguration, initialize_logging},
    },
    application::{crypto::key_manager::KeyManagerConfig, export_manager::ExportWriter},
    cli::{Cli, CoreCliService, dispatch},
    cli_recording::{PipelineRecordingControl, RecordingControlFiles},
    domain::models::DeviceId,
    pipeline_composition::{PipelineConfig, spawn_recording_controller},
    storage_composition::{EncryptedStores, open_encrypted_stores},
};

/// Initializes quiet logging, composes CLI dependencies, and runs one parsed command.
///
/// Inputs: Reads optional `NOTE_TAKER_DEVICE_ID`, `NOTE_TAKER_LOG_DIR`, `NOTE_TAKER_DATABASE_PATH`,
/// `NOTE_TAKER_EXPORT_DIR`, `NOTE_TAKER_ENCRYPTION_PASSPHRASE`, and
/// `NOTE_TAKER_ENCRYPTION_KEY_DIR` environment variables, the pipeline variables read by
/// `pipeline_composition` (for `record start`), plus standard clap command arguments.
/// Outputs: A process exit status after printing command output or a safe failure message.
/// Side effects: Creates logging resources, unlocks the embedding encryption key, opens the note
/// repository and the voice store (speaker names and voice variants) on the shared local SQLite
/// database, and may run a foreground recording through the meeting pipeline or invoke a
/// repository/voice application service according to the parsed command.
fn main() -> Result<()> {
    let device_id = DeviceId::new(
        env::var("NOTE_TAKER_DEVICE_ID").unwrap_or_else(|_| String::from("unconfigured-device")),
    );
    let log_directory = env::var("NOTE_TAKER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("logs"));
    let log_filter = String::from("error");
    let _logging_runtime = initialize_logging(
        LoggingConfiguration::new(device_id, log_directory).with_filter(log_filter),
    )?;
    let database_path = env::var("NOTE_TAKER_DATABASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("note_taker.sqlite3"));
    let store_device_id = DeviceId::new(
        env::var("NOTE_TAKER_DEVICE_ID").unwrap_or_else(|_| String::from("unconfigured-device")),
    );
    let EncryptedStores {
        note_repository: repository,
        voice_store,
        embedding_cipher,
    } = open_encrypted_stores(
        &database_path,
        store_device_id.clone(),
        KeyManagerConfig::from_environment(),
    )?;
    let recording_database_path = database_path.clone();
    let recording_control = PipelineRecordingControl::new(
        RecordingControlFiles::for_database(&database_path),
        Box::new(move || {
            spawn_recording_controller(
                &recording_database_path,
                store_device_id.clone(),
                Arc::clone(&embedding_cipher),
                PipelineConfig::from_environment()?,
            )
        }),
    );
    let export_writer = ExportWriter::new(default_exporters(), default_export_directory());
    let mut cli_service =
        CoreCliService::new(recording_control, repository, voice_store, export_writer);
    let parsed_cli = Cli::parse();
    let command_output =
        dispatch(parsed_cli.command, &mut cli_service).map_err(anyhow::Error::msg)?;
    println!("{command_output}");
    Ok(())
}
