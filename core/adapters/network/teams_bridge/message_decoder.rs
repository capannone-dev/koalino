//! Decodes one Teams bridge WebSocket frame (a serialized `TeamsBridgeMessage`) into domain values.
//!
//! This is the only place that touches the generated Protobuf types; everything downstream sees
//! `ExternalMediaChunk`, `SlideCapture`, `ExternalMeetingDocument`, and `BridgeMeeting`. Validation follows the protocol in
//! `core/protocol/README.md`: audio is 16 kHz mono 16-bit PCM of at most 30 s, media requires a
//! preceding `MeetingMetadata`, only screen-share frames become slides, documents are UTF-8 text
//! of a known kind of at most 4 MiB, and a meeting end must name the active meeting. Error reasons never
//! contain media bytes, e-mail addresses, or secrets.

use protobuf::Message;
use uuid::Uuid;

use crate::{
    domain::{
        errors::ExternalBridgeError,
        models::{
            ExternalMediaChunk, ExternalMeetingDocument, ExternalParticipant, ExternalTrackKind,
            MeetingDocumentKind, MeetingDocumentMediaType, MeetingId, SlideCapture,
        },
    },
    protocol::teams_bridge::{self, TeamsBridgeMessage, TrackKind, teams_bridge_message::Payload},
};

/// The only accepted audio sample rate.
pub const REQUIRED_SAMPLE_RATE_HZ: u32 = crate::domain::models::EXTERNAL_AUDIO_SAMPLE_RATE_HZ;
/// Longest accepted audio chunk; the bot should send 1–30 s chunks.
pub const MAXIMUM_AUDIO_CHUNK_MS: u64 = 30_000;
/// Bytes of 16 kHz mono 16-bit PCM per millisecond.
pub const AUDIO_BYTES_PER_MS: u64 = 32;
/// Largest accepted meeting document (a long meeting's transcript is well under 1 MiB).
pub const MAXIMUM_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
/// Namespace for deriving local meeting UUIDs from Teams meeting IDs (UUIDv5).
const TEAMS_MEETING_NAMESPACE: Uuid = Uuid::from_u128(0x6e6f_7465_7461_6b65_7274_6561_6d73_0001);
/// The video source that carries slides.
const SCREEN_SHARE_SOURCE: &str = "screen_share";
/// The video source that is recognized but ignored.
const CAMERA_SOURCE: &str = "camera";

/// A meeting announced by the bot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeMeeting {
    meeting_id: MeetingId,
    meeting_uuid: Uuid,
    teams_meeting_id: String,
    title: String,
    start_time_ms: u64,
    participants: Vec<ExternalParticipant>,
}

impl BridgeMeeting {
    /// Returns the local meeting ID (the UUID as a string).
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the local meeting UUID used by slide captures.
    pub fn meeting_uuid(&self) -> Uuid {
        self.meeting_uuid
    }

    /// Returns the Teams meeting ID as sent by the bot.
    pub fn teams_meeting_id(&self) -> &str {
        &self.teams_meeting_id
    }

    /// Returns the meeting title (may be empty).
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the meeting start as Unix epoch milliseconds.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the participants known at meeting start.
    pub fn participants(&self) -> &[ExternalParticipant] {
        &self.participants
    }
}

/// One participant's validated audio chunk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeAudio {
    media_chunk: ExternalMediaChunk,
    meeting_uuid: Uuid,
}

impl BridgeAudio {
    /// Borrows the domain chunk (its audio must never be logged).
    pub fn media_chunk(&self) -> &ExternalMediaChunk {
        &self.media_chunk
    }

    /// Takes the domain chunk.
    pub fn into_media_chunk(self) -> ExternalMediaChunk {
        self.media_chunk
    }

    /// Returns the local meeting the chunk belongs to.
    pub fn meeting_uuid(&self) -> Uuid {
        self.meeting_uuid
    }

    /// Returns the number of PCM samples.
    pub fn sample_count(&self) -> usize {
        self.media_chunk.audio_bytes().len() / 2
    }

    /// Returns the chunk duration in milliseconds (rounded down).
    pub fn duration_ms(&self) -> u64 {
        self.media_chunk.audio_bytes().len() as u64 / AUDIO_BYTES_PER_MS
    }
}

/// What one frame carried.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeEvent {
    /// A meeting started or resumed.
    MeetingStarted(BridgeMeeting),
    /// Audio for the active meeting.
    Audio(BridgeAudio),
    /// A screen-share frame for the active meeting.
    Slide(SlideCapture),
    /// A camera frame, which is not stored.
    IgnoredVideo,
    /// A text document of the active meeting (Issue #102).
    Document(ExternalMeetingDocument),
    /// The active meeting is over (its local UUID).
    MeetingEnded(Uuid),
}

/// Derives the stable local meeting UUID of a Teams meeting.
///
/// Inputs: The Teams meeting ID.
/// Outputs: A UUIDv5, identical for every call with the same ID (so reconnects resume one meeting).
/// Side effects: None.
pub fn local_meeting_uuid(teams_meeting_id: &str) -> Uuid {
    Uuid::new_v5(&TEAMS_MEETING_NAMESPACE, teams_meeting_id.as_bytes())
}

/// Decodes and validates one binary frame.
///
/// Inputs: The frame bytes and the active meeting (from the last `MeetingMetadata`), if any.
/// Outputs: The event, or `InvalidData` for malformed Protobuf, an empty envelope, unsupported
/// audio, an unknown video source, or media before any meeting.
/// Side effects: None.
pub fn decode_frame(
    frame: &[u8],
    active_meeting: Option<Uuid>,
) -> Result<BridgeEvent, ExternalBridgeError> {
    let message = TeamsBridgeMessage::parse_from_bytes(frame)
        .map_err(|_| invalid("frame is not a valid TeamsBridgeMessage"))?;
    match message.payload {
        Some(Payload::Metadata(metadata)) => decode_metadata(metadata),
        Some(Payload::Audio(audio)) => {
            decode_audio(audio, require_meeting(active_meeting, "audio")?)
        }
        Some(Payload::Video(video)) => {
            decode_video(video, require_meeting(active_meeting, "video")?)
        }
        Some(Payload::Document(document)) => {
            let meeting_uuid = match document.meeting_id.trim() {
                "" => require_meeting(active_meeting, "document")?,
                teams_meeting_id => local_meeting_uuid(teams_meeting_id),
            };
            decode_document(document, meeting_uuid)
        }
        Some(Payload::Ended(ended)) => {
            let active_meeting = require_meeting(active_meeting, "meeting end")?;
            if local_meeting_uuid(ended.meeting_id.trim()) != active_meeting {
                return Err(invalid("meeting end names a meeting that is not active"));
            }
            Ok(BridgeEvent::MeetingEnded(active_meeting))
        }
        None => Err(invalid("frame carries no payload")),
    }
}

/// Returns the active meeting or explains that media came too early.
fn require_meeting(active_meeting: Option<Uuid>, media: &str) -> Result<Uuid, ExternalBridgeError> {
    active_meeting.ok_or_else(|| invalid(&format!("{media} received before MeetingMetadata")))
}

/// Converts meeting metadata.
fn decode_metadata(
    metadata: teams_bridge::MeetingMetadata,
) -> Result<BridgeEvent, ExternalBridgeError> {
    let teams_meeting_id = metadata.meeting_id.trim().to_owned();
    if teams_meeting_id.is_empty() {
        return Err(invalid("MeetingMetadata.meeting_id is empty"));
    }
    let meeting_uuid = local_meeting_uuid(&teams_meeting_id);
    let participants = metadata
        .participants
        .into_iter()
        .filter_map(|participant| decode_participant(participant).ok())
        .collect();
    Ok(BridgeEvent::MeetingStarted(BridgeMeeting {
        meeting_id: MeetingId::new(meeting_uuid.to_string()),
        meeting_uuid,
        teams_meeting_id,
        title: metadata.meeting_title,
        start_time_ms: metadata.start_time_ms,
        participants,
    }))
}

/// Converts a participant; an empty display name falls back to the external ID.
fn decode_participant(
    participant: teams_bridge::ExternalParticipant,
) -> Result<ExternalParticipant, ExternalBridgeError> {
    let external_id = participant.external_id.trim().to_owned();
    if external_id.is_empty() {
        return Err(invalid("participant external_id is empty"));
    }
    let display_name = match participant.display_name.trim() {
        "" => external_id.clone(),
        display_name => display_name.to_owned(),
    };
    let email = Some(participant.email.trim().to_owned()).filter(|email| !email.is_empty());
    Ok(ExternalParticipant::new(external_id, display_name, email))
}

/// Converts and validates an audio chunk.
fn decode_audio(
    audio: teams_bridge::AudioChunk,
    meeting_uuid: Uuid,
) -> Result<BridgeEvent, ExternalBridgeError> {
    if audio.sample_rate != REQUIRED_SAMPLE_RATE_HZ {
        return Err(invalid("audio must be 16 kHz mono 16-bit PCM"));
    }
    if audio.audio_data.is_empty() || !audio.audio_data.len().is_multiple_of(2) {
        return Err(invalid("audio must contain whole 16-bit samples"));
    }
    if audio.audio_data.len() as u64 > MAXIMUM_AUDIO_CHUNK_MS * AUDIO_BYTES_PER_MS {
        return Err(invalid("audio chunks must not exceed 30 s"));
    }
    let track_kind = match audio.track.enum_value() {
        Ok(TrackKind::PARTICIPANT) => ExternalTrackKind::Participant,
        Ok(TrackKind::MIXED_REMOTE) => ExternalTrackKind::MixedRemote,
        Err(_) => return Err(invalid("audio has an unknown track kind")),
    };
    let participant = audio
        .participant
        .into_option()
        .ok_or_else(|| invalid("audio has no participant"))?;
    Ok(BridgeEvent::Audio(BridgeAudio {
        media_chunk: ExternalMediaChunk::new(
            decode_participant(participant)?,
            audio.audio_data,
            audio.timestamp_ms,
        )
        .with_track_kind(track_kind),
        meeting_uuid,
    }))
}

/// Converts and validates a meeting document.
fn decode_document(
    document: teams_bridge::MeetingDocument,
    meeting_uuid: Uuid,
) -> Result<BridgeEvent, ExternalBridgeError> {
    let kind = match document.kind.as_str() {
        "teams_transcript" => MeetingDocumentKind::TeamsTranscript,
        "teams_live_transcript" => MeetingDocumentKind::TeamsLiveTranscript,
        "teams_captions" => MeetingDocumentKind::TeamsCaptions,
        _ => return Err(invalid("document kind is not supported")),
    };
    let media_type = match document.media_type.as_str() {
        "text/vtt" => MeetingDocumentMediaType::WebVtt,
        "text/plain" => MeetingDocumentMediaType::PlainText,
        _ => return Err(invalid("document must be text/vtt or text/plain")),
    };
    if document.content.is_empty() {
        return Err(invalid("document is empty"));
    }
    if document.content.len() > MAXIMUM_DOCUMENT_BYTES {
        return Err(invalid("document exceeds 4 MiB"));
    }
    let text =
        String::from_utf8(document.content).map_err(|_| invalid("document is not UTF-8 text"))?;
    Ok(BridgeEvent::Document(ExternalMeetingDocument::new(
        meeting_uuid,
        kind,
        media_type,
        text,
        document.timestamp_ms,
    )))
}

/// Converts a video frame; only screen shares become slides.
fn decode_video(
    video: teams_bridge::VideoFrame,
    meeting_uuid: Uuid,
) -> Result<BridgeEvent, ExternalBridgeError> {
    match video.source.as_str() {
        CAMERA_SOURCE => Ok(BridgeEvent::IgnoredVideo),
        SCREEN_SHARE_SOURCE if !video.image_data.is_empty() => {
            Ok(BridgeEvent::Slide(SlideCapture::new(
                meeting_uuid,
                video.image_data,
                video.timestamp_ms,
                video.source,
            )))
        }
        SCREEN_SHARE_SOURCE => Err(invalid("screen-share frame has no image data")),
        _ => Err(invalid("video source must be screen_share or camera")),
    }
}

/// Builds an `InvalidData` error with a privacy-safe reason.
fn invalid(reason: &str) -> ExternalBridgeError {
    ExternalBridgeError::InvalidData {
        reason: reason.to_owned(),
    }
}
