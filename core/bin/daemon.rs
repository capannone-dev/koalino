//! Starts the headless daemon composition root with structured JSON logging enabled.

use std::{env, path::PathBuf};

use anyhow::Result;
use note_taker_core::{
    adapters::observability::{LoggingConfiguration, initialize_logging},
    domain::models::DeviceId,
};

/// Initializes daemon logging and reserves the executable boundary for server and FFI services.
///
/// Inputs: Reads optional `NOTE_TAKER_DEVICE_ID`, `NOTE_TAKER_LOG_DIR`, and `RUST_LOG` environment variables.
/// Outputs: A process exit status indicating whether startup succeeded.
/// Side effects: Creates a local log directory and installs JSON stdout/rolling-file logging.
fn main() -> Result<()> {
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
    tracing::info!("daemon scaffold started");
    Ok(())
}
