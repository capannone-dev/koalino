//! Implements the meeting-document schema: origin, transcript, and segments (Issue #43).
//!
//! Layout under the document root (the title and notes are handled by `meeting_crdt`):
//!
//! | Key | Value |
//! | --- | --- |
//! | `origin` | map `{ created_at_ms: int, acquisition_device_id: str }`, written once |
//! | `transcript` | string |
//! | `segments` | map from `<start_ms:012>-<end_ms:012>[-<n>]` to a segment map |
//! | `project_id` | string, the hyphenated UUID of the meeting's project (Issue #85); absent for none |
//!
//! Each segment map is `{ start_ms: uint, end_ms: uint, text: str, speaker_identity_id?: str,
//! unknown_cluster_id?: str }`, with UUIDs in hyphenated form. Keys are derived from the span, so
//! both devices address the same segment, and each field is its own CRDT register: a speaker
//! relink and a text fix made on different devices merge.
//!
//! `cluster_resolutions` (Issue #50) maps an unknown-speaker cluster ID to the identity ID it was
//! named as, both hyphenated UUID strings. One register per cluster, so two devices naming the
//! same cluster at once converge on Automerge's winner. A segment whose `unknown_cluster_id` is
//! resolved projects with that identity as its speaker, whatever its own `speaker_identity_id`
//! says, so every segment of the cluster follows the winner on every device.

use std::collections::{HashMap, HashSet};

use automerge::{
    AutoCommit, ObjId, ObjType, ROOT, ReadDoc, ScalarValue, Value, transaction::Transactable,
};
use chrono::{TimeZone, Utc};
use uuid::Uuid;

use crate::{
    domain::{
        errors::StorageError,
        models::{DeviceId, MeetingId, TranscriptSegment},
        projects::ProjectId,
    },
    ports::meeting_document::{MeetingDocumentCodec, MeetingOrigin, MeetingProjection},
};

/// Root key of the origin map.
const ORIGIN_KEY: &str = "origin";
/// Root key of the transcript text.
const TRANSCRIPT_KEY: &str = "transcript";
/// Root key of the segments map.
const SEGMENTS_KEY: &str = "segments";
/// Root key of the cluster-resolutions map.
const CLUSTER_RESOLUTIONS_KEY: &str = "cluster_resolutions";
/// Root key of the project the meeting belongs to, as a UUID string (Issue #85). It is a register
/// like the title: concurrent moves resolve to Automerge's deterministic winner. Absent means the
/// meeting is in no project.
const PROJECT_KEY: &str = "project_id";

/// The Automerge implementation of `MeetingDocumentCodec`.
#[derive(Clone, Copy, Debug, Default)]
pub struct AutomergeMeetingDocumentCodec;

impl MeetingDocumentCodec for AutomergeMeetingDocumentCodec {
    fn with_origin(
        &self,
        document: &[u8],
        origin: &MeetingOrigin,
    ) -> Result<Vec<u8>, StorageError> {
        let mut automerge_document = load(document)?;
        if automerge_document
            .get(ROOT, ORIGIN_KEY)
            .map_err(|error| crdt_error("unable to read the meeting origin", error))?
            .is_some()
        {
            return Ok(automerge_document.save());
        }
        let origin_object = automerge_document
            .put_object(ROOT, ORIGIN_KEY, ObjType::Map)
            .map_err(|error| crdt_error("unable to write the meeting origin", error))?;
        automerge_document
            .put(
                &origin_object,
                "created_at_ms",
                origin.created_at.timestamp_millis(),
            )
            .and_then(|()| {
                automerge_document.put(
                    &origin_object,
                    "acquisition_device_id",
                    origin.acquisition_device_id.as_str(),
                )
            })
            .map_err(|error| crdt_error("unable to write the meeting origin", error))?;
        Ok(automerge_document.save())
    }

    fn with_transcript(&self, document: &[u8], transcript: &str) -> Result<Vec<u8>, StorageError> {
        let mut automerge_document = load(document)?;
        if read_string(&automerge_document, &ROOT, TRANSCRIPT_KEY).as_deref() != Some(transcript) {
            automerge_document
                .put(ROOT, TRANSCRIPT_KEY, transcript)
                .map_err(|error| crdt_error("unable to write the transcript", error))?;
        }
        Ok(automerge_document.save())
    }

    fn with_segments(
        &self,
        document: &[u8],
        segments: &[TranscriptSegment],
    ) -> Result<Vec<u8>, StorageError> {
        let mut automerge_document = load(document)?;
        let segments_object = match automerge_document
            .get(ROOT, SEGMENTS_KEY)
            .map_err(|error| crdt_error("unable to read the segments", error))?
        {
            Some((Value::Object(ObjType::Map), object_id)) => object_id,
            _ => automerge_document
                .put_object(ROOT, SEGMENTS_KEY, ObjType::Map)
                .map_err(|error| crdt_error("unable to create the segments", error))?,
        };
        let keyed_segments = keyed(segments);
        let wanted_keys: HashSet<&str> =
            keyed_segments.iter().map(|(key, _)| key.as_str()).collect();
        let stale_keys: Vec<String> = automerge_document
            .keys(&segments_object)
            .filter(|key| !wanted_keys.contains(key.as_str()))
            .collect();
        for stale_key in stale_keys {
            automerge_document
                .delete(&segments_object, stale_key.as_str())
                .map_err(|error| crdt_error("unable to delete a segment", error))?;
        }
        for (key, segment) in &keyed_segments {
            write_segment(&mut automerge_document, &segments_object, key, segment)?;
        }
        Ok(automerge_document.save())
    }

    fn with_cluster_resolution(
        &self,
        document: &[u8],
        cluster_id: Uuid,
        identity_id: Uuid,
    ) -> Result<Vec<u8>, StorageError> {
        let mut automerge_document = load(document)?;
        let resolutions_object = match automerge_document
            .get(ROOT, CLUSTER_RESOLUTIONS_KEY)
            .map_err(|error| crdt_error("unable to read the cluster resolutions", error))?
        {
            Some((Value::Object(ObjType::Map), object_id)) => object_id,
            _ => automerge_document
                .put_object(ROOT, CLUSTER_RESOLUTIONS_KEY, ObjType::Map)
                .map_err(|error| crdt_error("unable to create the cluster resolutions", error))?,
        };
        let cluster_key = cluster_id.to_string();
        let identity_text = identity_id.to_string();
        if read_string(&automerge_document, &resolutions_object, &cluster_key).as_deref()
            != Some(identity_text.as_str())
        {
            automerge_document
                .put(&resolutions_object, cluster_key.as_str(), identity_text)
                .map_err(|error| crdt_error("unable to write a cluster resolution", error))?;
        }
        Ok(automerge_document.save())
    }

    fn with_project(
        &self,
        document: &[u8],
        project_id: Option<ProjectId>,
    ) -> Result<Vec<u8>, StorageError> {
        let mut automerge_document = load(document)?;
        let stored = read_string(&automerge_document, &ROOT, PROJECT_KEY);
        match project_id {
            Some(project_id) => {
                let project_text = project_id.to_string();
                if stored.as_deref() != Some(project_text.as_str()) {
                    automerge_document
                        .put(ROOT, PROJECT_KEY, project_text)
                        .map_err(|error| crdt_error("unable to write the project", error))?;
                }
            }
            None => {
                if automerge_document
                    .get(ROOT, PROJECT_KEY)
                    .ok()
                    .flatten()
                    .is_some()
                {
                    automerge_document
                        .delete(ROOT, PROJECT_KEY)
                        .map_err(|error| crdt_error("unable to clear the project", error))?;
                }
            }
        }
        Ok(automerge_document.save())
    }

    fn project(
        &self,
        meeting_id: &MeetingId,
        document: &[u8],
    ) -> Result<MeetingProjection, StorageError> {
        let automerge_document = load(document)?;
        let origin = match automerge_document.get(ROOT, ORIGIN_KEY) {
            Ok(Some((Value::Object(ObjType::Map), origin_object))) => {
                read_origin(&automerge_document, &origin_object)
            }
            _ => None,
        };
        let transcript = read_string(&automerge_document, &ROOT, TRANSCRIPT_KEY);
        let cluster_resolutions = match automerge_document.get(ROOT, CLUSTER_RESOLUTIONS_KEY) {
            Ok(Some((Value::Object(ObjType::Map), resolutions_object))) => {
                read_cluster_resolutions(&automerge_document, &resolutions_object)
            }
            _ => Vec::new(),
        };
        let resolved_speakers: HashMap<Uuid, Uuid> = cluster_resolutions.iter().copied().collect();
        let segments = match automerge_document.get(ROOT, SEGMENTS_KEY) {
            Ok(Some((Value::Object(ObjType::Map), segments_object))) => Some(
                read_segments(&automerge_document, &segments_object, meeting_id)
                    .into_iter()
                    .map(|segment| {
                        match segment
                            .unknown_cluster_id()
                            .and_then(|cluster_id| resolved_speakers.get(&cluster_id))
                        {
                            Some(identity_id) => {
                                segment.with_speaker_identity_id(Some(*identity_id))
                            }
                            None => segment,
                        }
                    })
                    .collect(),
            ),
            _ => None,
        };
        // A value that is not a project ID (from a faulty peer) leaves the meeting outside any.
        let project_id = read_string(&automerge_document, &ROOT, PROJECT_KEY)
            .and_then(|project_text| ProjectId::parse(&project_text));
        Ok(MeetingProjection {
            origin,
            transcript,
            segments,
            cluster_resolutions,
            project_id,
        })
    }
}

/// Assigns each segment its document key: the zero-padded span, with `-<n>` for repeated spans.
fn keyed(segments: &[TranscriptSegment]) -> Vec<(String, &TranscriptSegment)> {
    let mut span_occurrences: HashMap<(u64, u64), usize> = HashMap::new();
    segments
        .iter()
        .map(|segment| {
            let span = (segment.start_time_ms(), segment.end_time_ms());
            let occurrence = span_occurrences.entry(span).or_insert(0);
            let key = if *occurrence == 0 {
                format!("{:012}-{:012}", span.0, span.1)
            } else {
                format!("{:012}-{:012}-{occurrence}", span.0, span.1)
            };
            *occurrence += 1;
            (key, segment)
        })
        .collect()
}

/// Writes one segment, putting only the fields whose value differs.
fn write_segment(
    automerge_document: &mut AutoCommit,
    segments_object: &ObjId,
    key: &str,
    segment: &TranscriptSegment,
) -> Result<(), StorageError> {
    let segment_object = match automerge_document
        .get(segments_object, key)
        .map_err(|error| crdt_error("unable to read a segment", error))?
    {
        Some((Value::Object(ObjType::Map), object_id)) => object_id,
        _ => automerge_document
            .put_object(segments_object, key, ObjType::Map)
            .map_err(|error| crdt_error("unable to create a segment", error))?,
    };
    put_if_changed(
        automerge_document,
        &segment_object,
        "start_ms",
        ScalarValue::Uint(segment.start_time_ms()),
    )?;
    put_if_changed(
        automerge_document,
        &segment_object,
        "end_ms",
        ScalarValue::Uint(segment.end_time_ms()),
    )?;
    put_if_changed(
        automerge_document,
        &segment_object,
        "text",
        ScalarValue::Str(segment.text().into()),
    )?;
    for (field, value) in [
        ("speaker_identity_id", segment.speaker_identity_id()),
        ("unknown_cluster_id", segment.unknown_cluster_id()),
    ] {
        match value {
            Some(uuid) => put_if_changed(
                automerge_document,
                &segment_object,
                field,
                ScalarValue::Str(uuid.hyphenated().to_string().into()),
            )?,
            None => {
                if automerge_document
                    .get(&segment_object, field)
                    .map_err(|error| crdt_error("unable to read a segment field", error))?
                    .is_some()
                {
                    automerge_document
                        .delete(&segment_object, field)
                        .map_err(|error| crdt_error("unable to clear a segment field", error))?;
                }
            }
        }
    }
    Ok(())
}

/// Puts `value` at `field` unless it is already the current value.
fn put_if_changed(
    automerge_document: &mut AutoCommit,
    object: &ObjId,
    field: &str,
    value: ScalarValue,
) -> Result<(), StorageError> {
    let current = automerge_document
        .get(object, field)
        .map_err(|error| crdt_error("unable to read a segment field", error))?;
    if let Some((Value::Scalar(current_value), _)) = &current
        && current_value.as_ref() == &value
    {
        return Ok(());
    }
    automerge_document
        .put(object, field, value)
        .map_err(|error| crdt_error("unable to write a segment field", error))
}

/// Reads the origin map, or `None` when a field is missing or malformed.
fn read_origin(automerge_document: &AutoCommit, origin_object: &ObjId) -> Option<MeetingOrigin> {
    let created_at_ms = match read_scalar(automerge_document, origin_object, "created_at_ms")? {
        ScalarValue::Int(milliseconds) => milliseconds,
        ScalarValue::Uint(milliseconds) => i64::try_from(milliseconds).ok()?,
        _ => return None,
    };
    let created_at = Utc.timestamp_millis_opt(created_at_ms).single()?;
    let acquisition_device_id =
        read_string(automerge_document, origin_object, "acquisition_device_id")?;
    Some(MeetingOrigin {
        created_at,
        acquisition_device_id: DeviceId::new(acquisition_device_id),
    })
}

/// Reads every well-formed segment, ordered by start time then end time then key.
fn read_segments(
    automerge_document: &AutoCommit,
    segments_object: &ObjId,
    meeting_id: &MeetingId,
) -> Vec<TranscriptSegment> {
    let mut keyed_segments: Vec<(String, TranscriptSegment)> = Vec::new();
    for key in automerge_document.keys(segments_object) {
        let segment = match automerge_document.get(segments_object, key.as_str()) {
            Ok(Some((Value::Object(ObjType::Map), segment_object))) => {
                read_segment(automerge_document, &segment_object, meeting_id)
            }
            _ => None,
        };
        match segment {
            Some(segment) => keyed_segments.push((key, segment)),
            None => tracing::warn!("skipped a malformed transcript segment in a meeting document"),
        }
    }
    keyed_segments.sort_by(|(left_key, left), (right_key, right)| {
        (left.start_time_ms(), left.end_time_ms(), left_key).cmp(&(
            right.start_time_ms(),
            right.end_time_ms(),
            right_key,
        ))
    });
    keyed_segments
        .into_iter()
        .map(|(_, segment)| segment)
        .collect()
}

/// Reads the cluster-resolutions map, ordered by cluster; malformed entries are skipped with a
/// warning.
fn read_cluster_resolutions(
    automerge_document: &AutoCommit,
    resolutions_object: &ObjId,
) -> Vec<(Uuid, Uuid)> {
    let mut resolutions: Vec<(Uuid, Uuid)> = automerge_document
        .keys(resolutions_object)
        .filter_map(|cluster_key| {
            let resolution = Uuid::parse_str(&cluster_key).ok().zip(
                read_string(automerge_document, resolutions_object, &cluster_key)
                    .and_then(|identity_text| Uuid::parse_str(&identity_text).ok()),
            );
            if resolution.is_none() {
                tracing::warn!("skipped a malformed cluster resolution in a meeting document");
            }
            resolution
        })
        .collect();
    resolutions.sort();
    resolutions
}

/// Reads one segment map, or `None` when a required field is missing or malformed.
fn read_segment(
    automerge_document: &AutoCommit,
    segment_object: &ObjId,
    meeting_id: &MeetingId,
) -> Option<TranscriptSegment> {
    let start_ms = read_unsigned(automerge_document, segment_object, "start_ms")?;
    let end_ms = read_unsigned(automerge_document, segment_object, "end_ms")?;
    let text = read_string(automerge_document, segment_object, "text")?;
    let speaker_identity_id =
        read_optional_uuid(automerge_document, segment_object, "speaker_identity_id")?;
    let unknown_cluster_id =
        read_optional_uuid(automerge_document, segment_object, "unknown_cluster_id")?;
    Some(
        TranscriptSegment::new(
            meeting_id.clone(),
            start_ms,
            end_ms,
            speaker_identity_id,
            text,
        )
        .with_unknown_cluster_id(unknown_cluster_id),
    )
}

/// Reads a scalar field, or `None` when absent or not a scalar.
fn read_scalar(
    automerge_document: &AutoCommit,
    object: &ObjId,
    field: &str,
) -> Option<ScalarValue> {
    match automerge_document.get(object, field) {
        Ok(Some((Value::Scalar(value), _))) => Some(value.into_owned()),
        _ => None,
    }
}

/// Reads a string field, or `None` when absent or not a string.
fn read_string(automerge_document: &AutoCommit, object: &ObjId, field: &str) -> Option<String> {
    match read_scalar(automerge_document, object, field)? {
        ScalarValue::Str(text) => Some(text.to_string()),
        _ => None,
    }
}

/// Reads a non-negative integer field, or `None` when absent or malformed.
fn read_unsigned(automerge_document: &AutoCommit, object: &ObjId, field: &str) -> Option<u64> {
    match read_scalar(automerge_document, object, field)? {
        ScalarValue::Uint(value) => Some(value),
        ScalarValue::Int(value) => u64::try_from(value).ok(),
        _ => None,
    }
}

/// Reads an optional UUID field: `Some(None)` when absent, `None` when present but malformed.
fn read_optional_uuid(
    automerge_document: &AutoCommit,
    object: &ObjId,
    field: &str,
) -> Option<Option<Uuid>> {
    match automerge_document.get(object, field) {
        Ok(None) => Some(None),
        Ok(Some((Value::Scalar(value), _))) => match value.as_ref() {
            ScalarValue::Str(text) => Uuid::parse_str(text).ok().map(Some),
            _ => None,
        },
        _ => None,
    }
}

/// Deserializes a stored or received document.
fn load(document: &[u8]) -> Result<AutoCommit, StorageError> {
    AutoCommit::load(document).map_err(|error| crdt_error("meeting document is corrupt", error))
}

/// Converts an Automerge failure into a portable storage error.
fn crdt_error(operation: &str, error: automerge::AutomergeError) -> StorageError {
    StorageError::Rejected {
        reason: format!("{operation}: {error}"),
    }
}
