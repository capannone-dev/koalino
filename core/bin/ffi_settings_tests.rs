//! Specifies the settings the Flutter app passes to `note_taker_configure` (Issue #43).
//!
//! Each test uses its own `SettingsCell`, so no test touches the process-wide settings or the real
//! environment; the fallback is an injected closure.

use std::path::Path;

use super::{ConfiguredSettings, SettingsCell, SettingsError};

/// Parses a configuration that must be valid.
fn parsed(json: &str) -> ConfiguredSettings {
    ConfiguredSettings::from_json(json)
        .unwrap_or_else(|error| panic!("configuration should parse: {error}"))
}

/// Reads one setting with no fallback.
fn value(settings: &ConfiguredSettings, name: &str) -> Option<String> {
    settings.lookup(name, |_| None)
}

/// The data directory places the database, keys, logs, review clips, and exports inside it.
#[test]
fn the_data_directory_holds_every_file_the_core_writes() {
    let data_directory = Path::new("/data/user/0/app/files/note_taker");
    let settings = parsed(&format!(
        r#"{{"data_directory":"{}"}}"#,
        data_directory.display()
    ));

    for (name, file) in [
        ("NOTE_TAKER_DATABASE_PATH", "note_taker.sqlite3"),
        ("NOTE_TAKER_ENCRYPTION_KEY_DIR", "keys"),
        ("NOTE_TAKER_LOG_DIR", "logs"),
        ("NOTE_TAKER_REVIEW_CLIP_DIR", "voice_review_clips"),
        ("NOTE_TAKER_EXPORT_DIR", "exports"),
    ] {
        assert_eq!(
            value(&settings, name),
            Some(data_directory.join(file).display().to_string()),
            "{name}"
        );
    }
}

/// Optional fields map to the variables the composition roots already read.
#[test]
fn optional_fields_map_to_the_core_s_settings() {
    let settings = parsed(
        r#"{"data_directory":"/home/user/.local/share/note_taker","device_name":"Pixel 9",
            "whisper_model_path":"/models/ggml-base.bin","keychain_enabled":false,
            "p2p_enabled":true,"p2p_peers":["/ip4/10.0.0.2/tcp/4001/p2p/12D3KooWA","/dns4/home.example/tcp/4001/p2p/12D3KooWB"],
            "export_directory":"/storage/emulated/0/Documents/NoteTaker"}"#,
    );

    assert_eq!(
        value(&settings, "NOTE_TAKER_DEVICE_NAME").as_deref(),
        Some("Pixel 9")
    );
    assert_eq!(
        value(&settings, "NOTE_TAKER_WHISPER_MODEL").as_deref(),
        Some("/models/ggml-base.bin")
    );
    assert_eq!(
        value(&settings, "NOTE_TAKER_KEYCHAIN_ENABLED").as_deref(),
        Some("false")
    );
    assert_eq!(
        value(&settings, "NOTE_TAKER_P2P_ENABLED").as_deref(),
        Some("true")
    );
    assert_eq!(
        value(&settings, "NOTE_TAKER_P2P_PEERS").as_deref(),
        Some("/ip4/10.0.0.2/tcp/4001/p2p/12D3KooWA,/dns4/home.example/tcp/4001/p2p/12D3KooWB")
    );
    assert_eq!(
        value(&settings, "NOTE_TAKER_EXPORT_DIR").as_deref(),
        Some("/storage/emulated/0/Documents/NoteTaker")
    );
}

/// A configured value wins over the fallback; an unconfigured one falls back.
#[test]
fn configured_values_win_over_the_environment() {
    let settings = parsed(r#"{"data_directory":"/data","device_name":"Laptop"}"#);
    let fallback = |name: &str| {
        (name == "NOTE_TAKER_DEVICE_NAME" || name == "OPENAI_API_KEY")
            .then(|| format!("env {name}"))
    };

    assert_eq!(
        settings
            .lookup("NOTE_TAKER_DEVICE_NAME", fallback)
            .as_deref(),
        Some("Laptop")
    );
    assert_eq!(
        settings.lookup("OPENAI_API_KEY", fallback).as_deref(),
        Some("env OPENAI_API_KEY")
    );
    assert_eq!(settings.lookup("NOTE_TAKER_WHISPER_MODEL", fallback), None);
}

/// Malformed input is refused before anything is stored.
#[test]
fn invalid_configurations_are_refused() {
    for json in [
        "not json",
        r#"{}"#,
        r#"{"data_directory":""}"#,
        r#"{"data_directory":"relative/dir"}"#,
        r#"{"data_directory":"/data","unknown":1}"#,
        r#"{"data_directory":"/data","device_name":"   "}"#,
        r#"{"data_directory":"/data","whisper_model_path":"model.bin"}"#,
        r#"{"data_directory":"/data","p2p_peers":["/ip4/1.2.3.4/tcp/1/p2p/A,/ip4/5.6.7.8/tcp/1/p2p/B"]}"#,
    ] {
        assert!(
            matches!(
                ConfiguredSettings::from_json(json),
                Err(SettingsError::Invalid(_))
            ),
            "{json} must be refused"
        );
    }
}

/// The app configures once; a second configuration is refused and the first one stays.
#[test]
fn a_cell_accepts_one_configuration() {
    let cell = SettingsCell::new();
    assert_eq!(cell.lookup_with("NOTE_TAKER_DEVICE_NAME", |_| None), None);

    let first = cell.configure(parsed(r#"{"data_directory":"/a","device_name":"First"}"#));
    let second = cell.configure(parsed(r#"{"data_directory":"/b","device_name":"Second"}"#));

    assert!(first.is_ok());
    assert!(matches!(second, Err(SettingsError::AlreadyConfigured)));
    assert_eq!(
        cell.lookup_with("NOTE_TAKER_DEVICE_NAME", |_| None)
            .as_deref(),
        Some("First")
    );
}
