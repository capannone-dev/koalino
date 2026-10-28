//! Specifies the daemon's P2P configuration and task lifecycle (Issue #43).
//!
//! Configuration is read through an injected lookup, so the process environment is never used.
//! The task test listens on loopback only.

use std::{collections::HashMap, sync::Arc, time::Duration};

use uuid::Uuid;

use super::{P2pConfig, spawn_p2p_task};
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        p2p_service::{P2pCommand, P2pCommandOutcome},
    },
    domain::models::DeviceId,
};

/// Builds a config from literal variables, ignoring the real environment.
fn config_from(variables: &[(&str, &str)]) -> anyhow::Result<Option<P2pConfig>> {
    let variables: HashMap<String, String> = variables
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    P2pConfig::from_lookup(|name| variables.get(name).cloned())
}

/// Unwraps a configuration that must parse and be enabled.
fn enabled_config(variables: &[(&str, &str)]) -> P2pConfig {
    config_from(variables)
        .unwrap_or_else(|error| panic!("config should parse: {error:#}"))
        .unwrap_or_else(|| panic!("P2P should be enabled"))
}

/// With nothing set, P2P listens on every interface at a free port, named after the host.
#[test]
fn defaults_listen_everywhere_and_use_the_host_name() {
    let config = enabled_config(&[("HOSTNAME", "ettore-laptop")]);

    assert_eq!(config.listen_address, "/ip4/0.0.0.0/tcp/0");
    assert_eq!(config.device_name.as_str(), "ettore-laptop");
    assert!(config.static_peers.is_empty());
}

/// `NOTE_TAKER_P2P_PEERS` lists peer addresses separated by commas; blanks are ignored.
#[test]
fn configured_peer_addresses_are_split_and_trimmed() {
    let config = enabled_config(&[(
        "NOTE_TAKER_P2P_PEERS",
        " /ip4/10.0.0.2/tcp/4001/p2p/12D3KooWA , ,/ip4/10.0.0.3/tcp/4001/p2p/12D3KooWB",
    )]);

    assert_eq!(
        config.static_peers,
        vec![
            "/ip4/10.0.0.2/tcp/4001/p2p/12D3KooWA".to_owned(),
            "/ip4/10.0.0.3/tcp/4001/p2p/12D3KooWB".to_owned(),
        ]
    );
}

/// Explicit values win, and `NOTE_TAKER_P2P_ENABLED=false` turns P2P off.
#[test]
fn explicit_values_are_used_and_p2p_can_be_disabled() {
    let config = enabled_config(&[
        ("NOTE_TAKER_P2P_LISTEN", "/ip4/192.168.1.5/tcp/4001"),
        ("NOTE_TAKER_DEVICE_NAME", "Studio desktop"),
        ("HOSTNAME", "ignored"),
    ]);
    let disabled = config_from(&[("NOTE_TAKER_P2P_ENABLED", "false")])
        .unwrap_or_else(|error| panic!("config should parse: {error:#}"));

    assert_eq!(config.listen_address, "/ip4/192.168.1.5/tcp/4001");
    assert_eq!(config.device_name.as_str(), "Studio desktop");
    assert!(disabled.is_none());
}

/// An invalid explicit name or enable flag is an error, while an unusable host name falls back.
#[test]
fn invalid_explicit_values_fail_and_a_bad_host_name_falls_back() {
    assert!(config_from(&[("NOTE_TAKER_DEVICE_NAME", "   ")]).is_err());
    assert!(config_from(&[("NOTE_TAKER_P2P_ENABLED", "maybe")]).is_err());

    let fallback = enabled_config(&[("HOSTNAME", "\u{1b}[31m")]);

    assert_eq!(fallback.device_name.as_str(), "Note Taker device");
}

/// The task starts on loopback, reports its address and name, answers commands, and stops.
#[tokio::test]
async fn the_task_serves_status_and_commands_until_shutdown() {
    let directory = std::env::temp_dir().join(format!("p2p-task-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    let config = enabled_config(&[
        ("NOTE_TAKER_P2P_LISTEN", "/ip4/127.0.0.1/tcp/0"),
        ("NOTE_TAKER_DEVICE_NAME", "Test laptop"),
    ]);
    let mut misconfigured = config.clone();
    misconfigured.static_peers = vec!["/ip4/127.0.0.1/tcp/4001".to_owned()];
    let cipher = Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([4; 32])));

    let refused_start = spawn_p2p_task(
        &directory.join("misconfigured.sqlite3"),
        DeviceId::new("device-test"),
        Arc::clone(&cipher) as Arc<dyn crate::ports::crypto::EmbeddingCipher>,
        misconfigured,
    );
    assert!(
        refused_start.is_err(),
        "a peer address without a peer ID must stop start-up"
    );
    let handle = spawn_p2p_task(
        &directory.join("notes.sqlite3"),
        DeviceId::new("device-test"),
        cipher,
        config,
    )
    .unwrap_or_else(|error| panic!("task must start: {error:#}"));
    let mut listening = false;
    for _ in 0..100 {
        if !handle.snapshot().listen_addresses.is_empty() {
            listening = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let snapshot = handle.snapshot();
    let outcome = handle
        .command(P2pCommand::MakePairable)
        .await
        .unwrap_or_else(|error| panic!("command must succeed: {error:#}"));
    let refused = handle
        .command(P2pCommand::ConfirmCode(DeviceId::new("peer-unknown")))
        .await;
    tokio::task::spawn_blocking(move || handle.shutdown())
        .await
        .unwrap_or_else(|error| panic!("shutdown must not panic: {error}"));

    assert!(listening, "the task never reported a listen address");
    assert!(snapshot.listen_addresses[0].starts_with("/ip4/127.0.0.1/tcp/"));
    assert_eq!(snapshot.status.device_name.as_str(), "Test laptop");
    assert_eq!(outcome, P2pCommandOutcome::Done);
    assert!(refused.is_err());
}
