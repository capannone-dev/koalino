//! Formats every tracing event as one JSON object with the fields REQUIREMENTS.md 6.3 requires,
//! redacting sensitive values (6.4) before they reach stdout or the log file.
//!
//! ```json
//! {"timestamp":"2026-10-28T09:05:00.123456Z","level":"INFO","target":"p2p_network",
//!  "device_id":"device-…","message":"synced","context":{"peer_id":"xyz","latency_ms":45}}
//! ```
//!
//! `tracing-subscriber`'s built-in JSON format nests fields under `fields` and has no device ID,
//! so before this formatter only the start-up line identified the device.

use std::fmt;

use chrono::{SecondsFormat, Utc};
use serde_json::{Map, Value};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
    registry::LookupSpan,
};

use crate::domain::models::DeviceId;

/// Replacement written instead of a sensitive field's value.
const REDACTED_VALUE: &str = "[redacted]";

/// Field-name fragments whose values are never written: biometric data, key material,
/// credentials, and meeting content. Matching is case-insensitive and by substring, so it errs
/// towards redacting (`bridge_secret`, `meeting_title`, and `voice_embedding` all match).
const SENSITIVE_FIELD_FRAGMENTS: &[&str] = &[
    "embedding",
    "passphrase",
    "password",
    "secret",
    "token",
    "api_key",
    "encryption_key",
    "private_key",
    "key_material",
    "authorization",
    "credential",
    "title",
    "transcript",
    "raw_audio",
    "pcm",
];

/// Formats events as single-line JSON objects stamped with this process's device ID.
#[derive(Clone, Debug)]
pub struct StructuredJsonFormat {
    device_id: String,
}

impl StructuredJsonFormat {
    /// Creates a formatter stamping `device_id` on every entry.
    ///
    /// Inputs: The acquisition device ID (see `device_identity`). Outputs: The formatter.
    /// Side effects: None.
    pub fn new(device_id: &DeviceId) -> Self {
        Self {
            device_id: device_id.as_str().to_owned(),
        }
    }
}

impl<SubscriberType, FieldFormatter> FormatEvent<SubscriberType, FieldFormatter>
    for StructuredJsonFormat
where
    SubscriberType: Subscriber + for<'lookup> LookupSpan<'lookup>,
    FieldFormatter: for<'writer> FormatFields<'writer> + 'static,
{
    /// Writes `event` as `{timestamp, level, target, device_id, message, context?}` plus a newline.
    ///
    /// Inputs: The event and the writer for its sink. Outputs: `fmt::Error` only when the writer
    /// fails or the entry cannot be serialized. Side effects: Writes one line to the sink.
    fn format_event(
        &self,
        _context: &FmtContext<'_, SubscriberType, FieldFormatter>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut field_visitor = JsonFieldVisitor::default();
        event.record(&mut field_visitor);
        let metadata = event.metadata();
        let mut entry = Map::new();
        entry.insert(
            "timestamp".to_owned(),
            Value::String(Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)),
        );
        entry.insert(
            "level".to_owned(),
            Value::String(metadata.level().as_str().to_owned()),
        );
        entry.insert(
            "target".to_owned(),
            Value::String(metadata.target().to_owned()),
        );
        entry.insert(
            "device_id".to_owned(),
            Value::String(self.device_id.clone()),
        );
        entry.insert(
            "message".to_owned(),
            Value::String(field_visitor.message.unwrap_or_default()),
        );
        if !field_visitor.context.is_empty() {
            entry.insert("context".to_owned(), Value::Object(field_visitor.context));
        }
        let serialized_entry = serde_json::to_string(&entry).map_err(|_| fmt::Error)?;
        writeln!(writer, "{serialized_entry}")
    }
}

/// Collects an event's `message` and its other fields as typed JSON values.
#[derive(Default)]
struct JsonFieldVisitor {
    message: Option<String>,
    context: Map<String, Value>,
}

impl JsonFieldVisitor {
    /// Stores `value` under the field's name, or the redaction marker for a sensitive field.
    fn insert(&mut self, field: &Field, value: Value) {
        let value = if is_sensitive_field(field.name()) {
            Value::String(REDACTED_VALUE.to_owned())
        } else {
            value
        };
        self.context.insert(field.name().to_owned(), value);
    }
}

impl Visit for JsonFieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_owned());
        } else {
            self.insert(field, Value::String(value.to_owned()));
        }
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, Value::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.insert(field, Value::from(value));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.insert(field, Value::String(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.insert(field, Value::String(format!("{value:?}")));
        }
    }
}

/// Reports whether a field's value must be redacted, by case-insensitive name fragment.
fn is_sensitive_field(field_name: &str) -> bool {
    let field_name = field_name.to_ascii_lowercase();
    SENSITIVE_FIELD_FRAGMENTS
        .iter()
        .any(|fragment| field_name.contains(fragment))
}

#[cfg(test)]
#[path = "structured_json_tests.rs"]
mod tests;
