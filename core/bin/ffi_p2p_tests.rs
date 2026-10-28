//! Specifies the P2P JSON the Flutter Devices screen reads and sends through the FFI (Issue #43).
//!
//! The request checks run without a task; the pairing test runs two real P2P tasks that listen on
//! loopback and find each other through configured peer addresses, like the daemon VM tests.

use std::{collections::HashMap, sync::Arc, time::Duration};

use serde_json::Value;
use uuid::Uuid;

use super::{p2p_command_json, p2p_status_json};
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        p2p_service::P2pControl,
    },
    domain::models::DeviceId,
    p2p_composition::{P2pConfig, P2pHandle, spawn_p2p_task},
    ports::crypto::EmbeddingCipher,
};

/// Parses a JSON answer.
fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("answer must be JSON: {error}"))
}

/// Without a P2P task the status says so and nothing else.
#[test]
fn the_status_without_a_task_is_disabled() {
    assert_eq!(
        json(&p2p_status_json(None)),
        serde_json::json!({"enabled": false})
    );
}

/// Malformed requests are refused before they reach a task; a valid one without a task is
/// reported as unavailable.
#[test]
fn malformed_requests_are_refused_and_a_missing_task_is_unavailable() {
    for (request, error_code) in [
        ("not json", "invalid_request"),
        (r#"{"command":"format_disk"}"#, "invalid_request"),
        (r#"{"command":"start_pairing"}"#, "invalid_request"),
        (
            r#"{"command":"make_pairable","extra":1}"#,
            "invalid_request",
        ),
        (
            r#"{"command":"start_pairing","peer_id":"12D3 KooW"}"#,
            "invalid_peer_id",
        ),
        (r#"{"command":"revoke","peer_id":""}"#, "invalid_peer_id"),
        (r#"{"command":"make_pairable"}"#, "unavailable"),
        (
            r#"{"command":"confirm_code","peer_id":"12D3KooWA"}"#,
            "unavailable",
        ),
    ] {
        let answer = json(&p2p_command_json(None, request));
        assert_eq!(answer["ok"], false, "{request}");
        assert_eq!(answer["error_code"], error_code, "{request}");
    }
}

/// Starts a P2P task on loopback over a fresh database, with `static_peers` configured.
fn start_task(name: &str, static_peers: &[String]) -> P2pHandle {
    let directory = std::env::temp_dir().join(format!("ffi-p2p-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    let variables: HashMap<&str, String> = HashMap::from([
        ("NOTE_TAKER_P2P_LISTEN", "/ip4/127.0.0.1/tcp/0".to_owned()),
        ("NOTE_TAKER_DEVICE_NAME", name.to_owned()),
        ("NOTE_TAKER_P2P_PEERS", static_peers.join(",")),
    ]);
    let config = P2pConfig::from_lookup(|variable| variables.get(variable).cloned())
        .unwrap_or_else(|error| panic!("config should parse: {error:#}"))
        .unwrap_or_else(|| panic!("P2P should be enabled"));
    let cipher: Arc<dyn EmbeddingCipher> =
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([9; 32])));
    spawn_p2p_task(
        &directory.join("notes.sqlite3"),
        DeviceId::new(format!("device-{name}")),
        cipher,
        config,
    )
    .unwrap_or_else(|error| panic!("task must start: {error:#}"))
}

/// Polls `handle`'s status JSON until `condition` holds, for up to ten seconds.
fn wait_for_status(handle: &P2pHandle, condition: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..200 {
        let status = json(&p2p_status_json(Some(handle as &dyn P2pControl)));
        if condition(&status) {
            return status;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    json(&p2p_status_json(Some(handle as &dyn P2pControl)))
}

/// Runs one command through the JSON interface.
fn command(handle: &P2pHandle, request: &str) -> Value {
    json(&p2p_command_json(Some(handle as &dyn P2pControl), request))
}

/// Two devices pair through the JSON commands: one opens pairing, the other starts it, both show
/// the same code, and once both confirm each lists the other as paired.
#[test]
fn two_devices_pair_through_the_json_commands() {
    let laptop = start_task("laptop", &[]);
    let laptop_status = wait_for_status(&laptop, |status| {
        status["listen_addresses"]
            .as_array()
            .is_some_and(|addresses| !addresses.is_empty())
    });
    let laptop_id = laptop_status["local_peer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let laptop_address = format!(
        "{}/p2p/{laptop_id}",
        laptop_status["listen_addresses"][0]
            .as_str()
            .unwrap_or_default()
    );
    let phone = start_task("phone", &[laptop_address]);
    let phone_id = json(&p2p_status_json(Some(&phone as &dyn P2pControl)))["local_peer_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    let opened = command(&laptop, r#"{"command":"make_pairable"}"#);
    let nearby = wait_for_status(&phone, |status| {
        status["nearby_devices"].as_array().is_some_and(|devices| {
            devices
                .iter()
                .any(|device| device["peer_id"] == laptop_id.as_str())
        })
    });
    let started = command(
        &phone,
        &format!(r#"{{"command":"start_pairing","peer_id":"{laptop_id}"}}"#),
    );
    let requested = wait_for_status(&laptop, |status| {
        status["incoming_requests"]
            .as_array()
            .is_some_and(|requests| !requests.is_empty())
    });
    let confirmed = command(
        &phone,
        &format!(r#"{{"command":"confirm_code","peer_id":"{laptop_id}"}}"#),
    );
    let accepted = command(
        &laptop,
        &format!(r#"{{"command":"accept_request","peer_id":"{phone_id}"}}"#),
    );
    let phone_paired = wait_for_status(&phone, |status| {
        status["trusted_devices"]
            .as_array()
            .is_some_and(|devices| !devices.is_empty())
    });
    let revoked = command(
        &laptop,
        &format!(r#"{{"command":"revoke","peer_id":"{phone_id}"}}"#),
    );
    laptop.shutdown();
    phone.shutdown();

    assert_eq!(opened, serde_json::json!({"ok": true}));
    assert!(
        nearby["nearby_devices"]
            .as_array()
            .is_some_and(|devices| !devices.is_empty()),
        "the configured laptop must be offered for pairing: {nearby}"
    );
    assert_eq!(started["ok"], true, "{started}");
    let code = started["code"].as_str().unwrap_or_default();
    assert_eq!(code.len(), 6, "{started}");
    assert_eq!(
        requested["incoming_requests"][0]["code"], code,
        "{requested}"
    );
    assert_eq!(requested["incoming_requests"][0]["device_name"], "phone");
    assert_eq!(confirmed["ok"], true, "{confirmed}");
    assert_eq!(accepted["ok"], true, "{accepted}");
    assert_eq!(accepted["update"]["kind"], "paired", "{accepted}");
    assert_eq!(
        phone_paired["trusted_devices"][0]["peer_id"],
        laptop_id.as_str()
    );
    assert_eq!(phone_paired["trusted_devices"][0]["device_name"], "laptop");
    assert_eq!(revoked, serde_json::json!({"ok": true, "revoked": true}));
}
