//! Starts the interactive command-line composition root with quiet logging and injected services.

use std::{env, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use note_taker_core::{
    adapters::{
        audio::local_recorder::LocalRecorder,
        observability::{LoggingConfiguration, initialize_logging},
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
        },
    },
    cli::{Cli, CoreCliService, dispatch},
    domain::models::{AudioConfig, DeviceId},
};

/// Initializes quiet logging, composes CLI dependencies, and runs one parsed command.
///
/// Inputs: Reads optional `NOTE_TAKER_DEVICE_ID`, `NOTE_TAKER_LOG_DIR`, and
/// `NOTE_TAKER_DATABASE_PATH` environment variables plus standard clap command arguments.
/// Outputs: A process exit status after printing command output or a safe failure message.
/// Side effects: Creates logging resources, opens the shared local SQLite database, and may invoke
/// a recorder or repository/voice application service according to the parsed command.
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
    let repository = SqliteNoteRepository::open(
        database_path,
        DeviceId::new(
            env::var("NOTE_TAKER_DEVICE_ID")
                .unwrap_or_else(|_| String::from("unconfigured-device")),
        ),
    )?;
    let recorder = LocalRecorder::new(AudioConfig::new(16_000, 1))?;
    let mut cli_service = CoreCliService::new(recorder, repository, InMemoryVoiceVectorDB::new());
    let parsed_cli = Cli::parse();
    let command_output =
        dispatch(parsed_cli.command, &mut cli_service).map_err(anyhow::Error::msg)?;
    println!("{command_output}");
    Ok(())
}
