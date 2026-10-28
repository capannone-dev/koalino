//! Specifies the JSON log line format required by REQUIREMENTS.md section 6.3, and its redaction.

use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::Value;
use tracing_subscriber::fmt::MakeWriter;

use super::StructuredJsonFormat;
use crate::domain::models::DeviceId;

/// Collects formatted log output in memory.
#[derive(Clone, Default)]
struct CapturedOutput(Arc<Mutex<Vec<u8>>>);

impl io::Write for CapturedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("capture lock poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for CapturedOutput {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

/// Runs `emit` under a subscriber using the structured format and returns each line as JSON.
fn captured_entries(emit: impl FnOnce()) -> Vec<Value> {
    let output = CapturedOutput::default();
    let subscriber = tracing_subscriber::fmt()
        .event_format(StructuredJsonFormat::new(&DeviceId::new("device-test")))
        .with_writer(output.clone())
        .with_max_level(tracing::Level::TRACE)
        .finish();
    tracing::subscriber::with_default(subscriber, emit);
    let bytes = output
        .0
        .lock()
        .unwrap_or_else(|_| panic!("capture lock must not be poisoned"))
        .clone();
    String::from_utf8(bytes)
        .unwrap_or_else(|error| panic!("log output must be UTF-8: {error}"))
        .lines()
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("{line:?} must be one JSON object: {error}"))
        })
        .collect()
}

/// Verifies every entry carries the required top-level fields, including the device ID.
#[test]
fn every_entry_has_the_required_metadata() {
    let entries = captured_entries(|| {
        tracing::info!("first event");
        tracing::debug!(target: "p2p_network", "second event");
    });

    assert_eq!(entries.len(), 2);
    for entry in &entries {
        assert_eq!(entry["device_id"], "device-test");
        let timestamp = entry["timestamp"]
            .as_str()
            .unwrap_or_else(|| panic!("timestamp must be a string: {entry}"));
        assert!(
            chrono::DateTime::parse_from_rfc3339(timestamp).is_ok(),
            "{timestamp}"
        );
    }
    assert_eq!(entries[0]["level"], "INFO");
    assert_eq!(entries[0]["message"], "first event");
    assert_eq!(entries[1]["level"], "DEBUG");
    assert_eq!(entries[1]["target"], "p2p_network");
}

/// Verifies structured fields land in a `context` object with their JSON types.
#[test]
fn fields_are_grouped_in_a_typed_context_object() {
    let entries = captured_entries(|| {
        tracing::info!(
            peer_id = "xyz",
            latency_ms = 45_u64,
            retry = true,
            ratio = 0.5,
            "synced"
        );
    });

    assert_eq!(entries[0]["message"], "synced");
    assert_eq!(
        entries[0]["context"],
        serde_json::json!({"peer_id": "xyz", "latency_ms": 45, "retry": true, "ratio": 0.5})
    );
}

/// Verifies an event without fields has no `context` key rather than an empty object.
#[test]
fn an_event_without_fields_has_no_context() {
    let entries = captured_entries(|| tracing::warn!("plain"));

    assert!(entries[0].get("context").is_none());
}

/// Verifies values of sensitive fields are replaced before anything is written.
#[test]
fn sensitive_fields_are_redacted() {
    let entries = captured_entries(|| {
        tracing::info!(
            voice_embedding = "0.12,0.34",
            encryption_key = "abcd",
            passphrase = "hunter2",
            api_key = "sk-live",
            bridge_secret = "s3cret",
            meeting_title = "Layoffs",
            transcript_text = "confidential words",
            authorization = "Bearer token",
            meeting_id = "meeting-1",
            "configured"
        );
    });

    let context = &entries[0]["context"];
    for sensitive_field in [
        "voice_embedding",
        "encryption_key",
        "passphrase",
        "api_key",
        "bridge_secret",
        "meeting_title",
        "transcript_text",
        "authorization",
    ] {
        assert_eq!(context[sensitive_field], "[redacted]", "{sensitive_field}");
    }
    assert_eq!(context["meeting_id"], "meeting-1");
    let line = entries[0].to_string();
    for secret in [
        "0.12",
        "abcd",
        "hunter2",
        "sk-live",
        "s3cret",
        "Layoffs",
        "confidential",
        "Bearer",
    ] {
        assert!(!line.contains(secret), "{secret} leaked into {line}");
    }
}

/// Verifies `Debug`-formatted values and errors are captured as strings.
#[test]
fn debug_and_error_values_are_strings() {
    let entries = captured_entries(|| {
        let failure = std::io::Error::other("disk full");
        tracing::error!(error = %failure, state = ?Some(3), "write failed");
    });

    assert_eq!(entries[0]["context"]["error"], "disk full");
    assert_eq!(entries[0]["context"]["state"], "Some(3)");
}
