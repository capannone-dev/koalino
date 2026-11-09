//! The Note Taker browser extension's native messaging host (Issue #102).
//!
//! Browsers start this program when the extension connects, passing their own arguments (Chrome
//! the extension origin, Firefox the manifest path and add-on ID), which are ignored. It reads
//! the extension's JSON messages on stdin and sends them as Teams bridge frames to the local
//! daemon (`NOTE_TAKER_TEAMS_BRIDGE_ENABLED=true`) over `ws://127.0.0.1:<port>`.
//!
//! `browser-host install` registers the host with Firefox, Chrome, Chromium and Edge for the
//! current user and creates the bridge secret in the keychain when the keychain is available.
//!
//! | Variable | Meaning | Default |
//! | --- | --- | --- |
//! | `NOTE_TAKER_TEAMS_BRIDGE_PORT` | the daemon's bridge port | `8765` |
//! | `NOTE_TAKER_TEAMS_BRIDGE_SECRET` | the bridge secret (never logged) | the keychain's `teams-bridge-secret` |
//!
//! Logs go to stderr (stdout carries the protocol) and never contain meeting content.

use std::{ffi::OsString, path::PathBuf, process::ExitCode};

use note_taker_core::adapters::{
    keychain::bridge_secret::{bridge_secret_keychain, ensure_bridge_secret, load_bridge_secret},
    network::{
        browser_host::{
            bridge_client::BridgeClientConfig,
            host_install::{
                CHROMIUM_EXTENSION_ID, FIREFOX_EXTENSION_ID, install_linux, install_windows,
            },
            host_session::run_host_session,
        },
        teams_bridge::teams_websocket_server::DEFAULT_TEAMS_BRIDGE_PORT,
    },
};

/// The variable naming the bridge port.
const PORT_VARIABLE: &str = "NOTE_TAKER_TEAMS_BRIDGE_PORT";
/// The variable holding the bridge secret.
const SECRET_VARIABLE: &str = "NOTE_TAKER_TEAMS_BRIDGE_SECRET";

/// Runs the host, or `install`.
///
/// Inputs: The command-line arguments.
/// Outputs: Success, or failure after printing the reason to stderr.
/// Side effects: See `run_host` and `install`.
fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    match arguments.first().and_then(|argument| argument.to_str()) {
        Some("install") => install(),
        Some("--help" | "-h") => {
            println!(
                "Usage: browser-host install   register the Note Taker extension's host with the browsers\n\
                 Browsers start browser-host themselves; it then reads native messages on stdin."
            );
            ExitCode::SUCCESS
        }
        _ => run_host(),
    }
}

/// Runs one native messaging session.
///
/// Side effects: Logs to stderr; reads stdin; writes stdout; connects to 127.0.0.1; may read the
/// keychain.
fn run_host() -> ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
    let port = match std::env::var(PORT_VARIABLE)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        None => DEFAULT_TEAMS_BRIDGE_PORT,
        Some(value) => match value.trim().parse::<u16>() {
            Ok(port) if port != 0 => port,
            _ => {
                eprintln!("{PORT_VARIABLE} must be a port between 1 and 65535");
                return ExitCode::FAILURE;
            }
        },
    };
    let secret = std::env::var(SECRET_VARIABLE)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| match load_bridge_secret(&bridge_secret_keychain()) {
            Ok(secret) => secret.map(|secret| secret.as_str().to_owned()),
            Err(error) => {
                tracing::warn!(error = %error, "Bridge secret unavailable; connecting without it");
                None
            }
        });
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("unable to start the host runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(
        port,
        authenticates = secret.is_some(),
        "Note Taker browser host started"
    );
    runtime.block_on(run_host_session(
        std::io::stdin(),
        std::io::stdout(),
        BridgeClientConfig::new(port, secret),
    ));
    tracing::info!("Note Taker browser host stopped");
    ExitCode::SUCCESS
}

/// Registers the host with the browsers and makes sure a bridge secret exists.
///
/// Side effects: Writes manifest files (and, on Windows, registry keys); may write the keychain;
/// prints what was done (never the secret).
fn install() -> ExitCode {
    let host_path = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("unable to find this program's path: {error}");
            return ExitCode::FAILURE;
        }
    };
    let written = if cfg!(windows) {
        install_windows(&host_path)
    } else {
        match std::env::var_os("HOME").map(PathBuf::from) {
            Some(home) => install_linux(&host_path, &home),
            None => {
                eprintln!("HOME is not set");
                return ExitCode::FAILURE;
            }
        }
    };
    match written {
        Ok(manifests) => {
            for manifest in manifests {
                println!("Registered: {}", manifest.display());
            }
        }
        Err(error) => {
            eprintln!("Registration failed: {error:#}");
            return ExitCode::FAILURE;
        }
    }
    match ensure_bridge_secret(&bridge_secret_keychain()) {
        Ok(_secret) => println!(
            "Bridge secret: in the keychain (note-taker / teams-bridge-secret); the daemon reads it too"
        ),
        Err(error) => println!(
            "Bridge secret: not created ({error}); the daemon's bridge then accepts any local program"
        ),
    }
    println!("Firefox add-on ID: {FIREFOX_EXTENSION_ID}");
    println!("Chrome/Edge extension ID: {CHROMIUM_EXTENSION_ID}");
    ExitCode::SUCCESS
}
