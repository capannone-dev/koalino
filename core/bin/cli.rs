//! Starts the interactive command-line composition root with quiet logging and injected services.

use std::{env, path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::Parser;
use note_taker_core::{
    adapters::{
        export::{default_export_directory, default_exporters},
        observability::{LogLevelControlFile, LoggingConfiguration, initialize_logging},
    },
    application::{crypto::key_manager::KeyManagerConfig, export_manager::ExportWriter},
    cli::{Cli, CoreCliService, dispatch},
    cli_recording::{PipelineRecordingControl, RecordingControlFiles},
    device_identity::device_id_from_environment,
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
    // Parsed first so `--help`, `--version`, and usage errors never open the database, and so
    // `--log-level` applies to this command's own logging.
    let parsed_cli = Cli::parse();
    let database_path = env::var("NOTE_TAKER_DATABASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("note_taker.sqlite3"));
    let device_id = device_id_from_environment(&database_path)?;
    let log_directory = env::var("NOTE_TAKER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("logs"));
    // `error` by default: the CLI prints JSON logs to stdout, which would mix with command output.
    let log_filter = parsed_cli
        .log_level
        .clone()
        .unwrap_or_else(|| String::from("error"));
    let _logging_runtime = initialize_logging(
        LoggingConfiguration::new(device_id.clone(), log_directory).with_filter(log_filter),
    )?;
    let store_device_id = device_id;
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
        CoreCliService::new(recording_control, repository, voice_store, export_writer)
            .with_log_level_control(LogLevelControlFile::for_database(&database_path));
    let command_output =
        dispatch(parsed_cli.command, &mut cli_service).map_err(anyhow::Error::msg)?;
    println!("{command_output}");
    Ok(())
}
