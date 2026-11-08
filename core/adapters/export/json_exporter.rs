//! Renders the stable, versioned JSON meeting-export schema for external tool integration.
//!
//! The schema (version `1.0`) is documented in `core/adapters/export/README.md`. It always wraps
//! meetings in a `meetings` array so single and batch exports share one shape, and it never
//! contains voice embeddings or audio.

use std::{collections::BTreeMap, path::Path};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    domain::{
        errors::ExportError,
        models::{ExportDocument, ExportFormat, ExportOptions, ExportResult},
    },
    ports::export::MeetingExporter,
};

use super::{EXPORT_SCHEMA_VERSION, ensure_documents_present, write_export_artifact};

/// Produces the versioned JSON representation of application-extracted meeting data.
pub struct JsonMeetingExporter;

/// Top-level JSON object.
#[derive(Serialize)]
struct JsonExportEnvelope<'document> {
    schema_version: &'static str,
    exported_at: String,
    meetings: Vec<JsonMeeting<'document>>,
}

/// One exported meeting.
#[derive(Serialize)]
struct JsonMeeting<'document> {
    meeting: JsonMeetingMetadata<'document>,
    transcript_segments: Vec<JsonTranscriptSegment<'document>>,
    attachments: Vec<JsonAttachment<'document>>,
    voice_identities: Vec<JsonSpeaker<'document>>,
    statistics: JsonStatistics,
}

/// Meeting metadata; only `meeting_id` is present when metadata is disabled.
#[derive(Serialize)]
struct JsonMeetingMetadata<'document> {
    meeting_id: &'document str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'document str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
}

/// One transcript segment with millisecond offsets from the meeting start.
#[derive(Serialize)]
struct JsonTranscriptSegment<'document> {
    start_time_ms: u64,
    end_time_ms: u64,
    speaker: JsonSpeaker<'document>,
    text: &'document str,
    /// The segment lies largely in overlapping speech (#90).
    overlapping_speech: bool,
    /// The speaker is a guess (#91).
    uncertain_speaker: bool,
    /// Positions of the unclear words among the text's whitespace-separated words (#91).
    uncertain_words: &'document [u16],
}

/// A speaker reference; `identity_id` is `null` for unattributed segments.
#[derive(Serialize)]
struct JsonSpeaker<'document> {
    identity_id: Option<String>,
    display_name: &'document str,
}

/// Attachment metadata with the original source path.
#[derive(Serialize)]
struct JsonAttachment<'document> {
    attachment_id: String,
    file_path: &'document str,
    mime_type: &'document str,
    original_timestamp: String,
}

/// Derived statistics.
#[derive(Serialize)]
struct JsonStatistics {
    word_count: usize,
    speaking_time_ms: BTreeMap<String, u64>,
}

impl JsonMeetingExporter {
    /// Serializes meetings into schema version `1.0`.
    ///
    /// Inputs: `documents` to include; `options` controls metadata fields; `exported_at` is the
    /// export instant recorded in the envelope.
    /// Outputs: Pretty-printed UTF-8 JSON bytes or `ExportError::Rendering`.
    /// Side effects: None; attachment files are not opened.
    pub fn render(
        documents: &[ExportDocument],
        options: &ExportOptions,
        exported_at: DateTime<Utc>,
    ) -> Result<Vec<u8>, ExportError> {
        let envelope = JsonExportEnvelope {
            schema_version: EXPORT_SCHEMA_VERSION,
            exported_at: exported_at.to_rfc3339(),
            meetings: documents
                .iter()
                .map(|document| json_meeting(document, options))
                .collect(),
        };
        serde_json::to_vec_pretty(&envelope).map_err(|error| ExportError::Rendering {
            reason: format!("unable to serialize JSON export: {error}"),
        })
    }
}

impl MeetingExporter for JsonMeetingExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Json
    }

    /// Writes rendered JSON atomically to `output_path`.
    ///
    /// Inputs: Non-empty `documents`, export `options`, and the destination path.
    /// Outputs: The written artifact summary or an `ExportError`.
    /// Side effects: Creates the destination directory and file.
    fn export_meetings(
        &self,
        documents: &[ExportDocument],
        options: &ExportOptions,
        output_path: &Path,
    ) -> Result<ExportResult, ExportError> {
        ensure_documents_present(documents.len())?;
        let rendered_json = Self::render(documents, options, Utc::now())?;
        write_export_artifact(
            output_path,
            &rendered_json,
            ExportFormat::Json,
            documents.len(),
        )
    }
}

/// Maps one export document onto the JSON schema.
///
/// Inputs: The `document` and the export `options`.
/// Outputs: The serializable meeting value borrowing from `document`.
/// Side effects: None.
fn json_meeting<'document>(
    document: &'document ExportDocument,
    options: &ExportOptions,
) -> JsonMeeting<'document> {
    let include_metadata = options.include_metadata();
    JsonMeeting {
        meeting: JsonMeetingMetadata {
            meeting_id: document.meeting_id().as_str(),
            title: include_metadata.then(|| document.title()),
            started_at: include_metadata.then(|| document.started_at().to_rfc3339()),
            duration_ms: include_metadata.then(|| document.duration_ms()),
        },
        transcript_segments: document
            .transcript_segments()
            .iter()
            .map(|segment| JsonTranscriptSegment {
                start_time_ms: segment.start_time_ms(),
                end_time_ms: segment.end_time_ms(),
                speaker: JsonSpeaker {
                    identity_id: segment
                        .speaker_identity_id()
                        .map(|identity_id| identity_id.to_string()),
                    display_name: segment.speaker(),
                },
                text: segment.text(),
                overlapping_speech: segment.confidence().overlapping_speech,
                uncertain_speaker: segment.confidence().uncertain_speaker,
                uncertain_words: &segment.confidence().uncertain_words,
            })
            .collect(),
        attachments: document
            .attachments()
            .iter()
            .map(|attachment| JsonAttachment {
                attachment_id: attachment.attachment_id().to_string(),
                file_path: attachment.file_path(),
                mime_type: attachment.mime_type(),
                original_timestamp: attachment.original_timestamp().to_rfc3339(),
            })
            .collect(),
        voice_identities: document
            .voice_identities()
            .iter()
            .map(|identity| JsonSpeaker {
                identity_id: Some(identity.identity_id().to_string()),
                display_name: identity.display_name(),
            })
            .collect(),
        statistics: JsonStatistics {
            word_count: document.word_count(),
            speaking_time_ms: document.speaking_time_by_speaker().into_iter().collect(),
        },
    }
}
