//! Starts the read-only MCP server (#112) that the owner's AI assistant runs over stdio.
//!
//! An assistant's MCP configuration starts this binary with the app's database path; Settings >
//! AI assistant access shows the exact command. The server answers until the assistant closes
//! its input. Standard output carries only protocol messages, so logs go to the log file alone.

use std::{env, io, path::PathBuf};

use anyhow::Result;
use note_taker_core::{
    adapters::observability::{LoggingConfiguration, initialize_logging},
    application::crypto::key_manager::KeyManagerConfig,
    cli_recording::LocalMeetingFiles,
    device_identity::device_id_from_environment,
    mcp_server::{AssistantAccessSettings, McpServer, serve},
    project_commands::ProjectFiles,
    storage_composition::{EncryptedStores, open_encrypted_stores},
};

/// Opens the notes and serves MCP on stdin and stdout.
///
/// Inputs: `NOTE_TAKER_DATABASE_PATH` (the app's database; `note_taker.sqlite3` otherwise),
/// `NOTE_TAKER_LOG_DIR` (default: `logs` beside the database), and the key variables read by
/// `KeyManagerConfig` (the platform keychain by default, as the app uses).
/// Outputs: Success when the assistant closes the input; an error when the database does not
/// exist (nothing is created) or the notes cannot be opened. Side effects: Unlocks the database key, opens the stores, writes the log file, and
/// reads notes for each tool call while access is turned on. It never writes notes.
fn main() -> Result<()> {
    let database_path = env::var("NOTE_TAKER_DATABASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("note_taker.sqlite3"));
    let log_directory = env::var("NOTE_TAKER_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            database_path
                .parent()
                .map(|directory| directory.join("logs"))
                .unwrap_or_else(|| PathBuf::from("logs"))
        });
    // A wrong path must not create an empty database (and a new key) beside the app's.
    if !database_path.is_file() {
        anyhow::bail!(
            "no notes at {}: set NOTE_TAKER_DATABASE_PATH to the path Koalino's Settings show",
            database_path.display()
        );
    }
    let device_id = device_id_from_environment(&database_path)?;
    let _logging_runtime = initialize_logging(
        LoggingConfiguration::new(device_id.clone(), log_directory)
            .with_filter(String::from("info"))
            .with_stdout_filter(String::from("off")),
    )?;
    let EncryptedStores {
        note_repository: repository,
        voice_store,
        ..
    } = open_encrypted_stores(
        &database_path,
        device_id.clone(),
        KeyManagerConfig::from_environment(),
    )?;
    let files = ProjectFiles::new(
        LocalMeetingFiles::for_database(&database_path)
            .attachment_directory()
            .to_path_buf(),
        device_id,
        repository.sealed_files(),
    );
    let settings_file = AssistantAccessSettings::file_for(&database_path);
    let server = McpServer::new(
        &repository,
        &voice_store,
        &files,
        Box::new(move || AssistantAccessSettings::load(&settings_file).enabled),
    );
    tracing::info!("assistant access server started");
    serve(&server, io::stdin().lock(), io::stdout().lock())?;
    tracing::info!("assistant access server stopped");
    Ok(())
}
