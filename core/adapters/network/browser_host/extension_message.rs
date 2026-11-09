//! The JSON messages the browser extension sends its native messaging host, and their
//! conversion into serialized `TeamsBridgeMessage` frames.
//!
//! | `type` | Fields | Frame |
//! | --- | --- | --- |
//! | `meeting` | `meetingId`, `title`, `startTimeMs`, `participants[{id,name}]` | `MeetingMetadata` |
//! | `audio` | `track` (`microphone` or `remote`), `participant{id,name}` (microphone only), `timestampMs`, `pcm` (base64 16 kHz mono s16le) | `AudioChunk` |
//! | `slide` | `timestampMs`, `image` (base64 PNG or JPEG) | `VideoFrame` (`screen_share`) |
//! | `document` | `kind`, `mediaType`, `text`, `timestampMs`, `meetingId` (optional) | `MeetingDocument` |
//! | `ended` | `meetingId` | `MeetingEnded` |
//!
//! The host checks the shape only; the core validates the content (sample rate, lengths, kinds).
//! Error reasons never contain message content.

use base64::Engine as _;
use protobuf::Message as _;
use serde::Deserialize;
use thiserror::Error;

use crate::protocol::teams_bridge::{
    AudioChunk, ExternalParticipant, MeetingDocument, MeetingEnded, MeetingMetadata,
    TeamsBridgeMessage, TrackKind, VideoFrame,
};

/// The external ID under which the mixed remote track is sent (never mapped to an identity).
pub const REMOTE_MIX_PARTICIPANT_ID: &str = "browser-remote-mix";
/// The display name of the mixed remote track.
const REMOTE_MIX_PARTICIPANT_NAME: &str = "Remote participants";
/// The only sample rate the extension sends.
const SAMPLE_RATE_HZ: u32 = 16_000;
/// Bytes of 16 kHz mono 16-bit PCM per millisecond.
const PCM_BYTES_PER_MS: u64 = 32;

/// Why a message could not be converted.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum ExtensionMessageError {
    /// The message does not have the expected shape.
    #[error("invalid extension message: {reason}")]
    Invalid {
        /// A privacy-safe reason.
        reason: String,
    },
}

/// One serialized bridge frame and what the queue needs to know about it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeFrame {
    bytes: Vec<u8>,
    audio_duration_ms: Option<u64>,
}

impl BridgeFrame {
    /// The serialized `TeamsBridgeMessage`. Side effects: None.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The audio length for an audio frame (the queue may drop these), `None` for any other
    /// frame (never dropped). Side effects: None.
    pub fn audio_duration_ms(&self) -> Option<u64> {
        self.audio_duration_ms
    }
}

/// A participant as the extension names it.
#[derive(Deserialize)]
struct ParticipantJson {
    id: String,
    name: String,
}

/// The extension's messages.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ExtensionMessage {
    /// A meeting started or was resumed.
    #[serde(rename_all = "camelCase")]
    Meeting {
        meeting_id: String,
        #[serde(default)]
        title: String,
        start_time_ms: u64,
        #[serde(default)]
        participants: Vec<ParticipantJson>,
    },
    /// One chunk of the microphone or of the mixed remote audio.
    #[serde(rename_all = "camelCase")]
    Audio {
        track: AudioTrack,
        participant: Option<ParticipantJson>,
        timestamp_ms: u64,
        pcm: String,
    },
    /// A cropped slide.
    #[serde(rename_all = "camelCase")]
    Slide { timestamp_ms: u64, image: String },
    /// A text document of the meeting.
    #[serde(rename_all = "camelCase")]
    Document {
        kind: String,
        media_type: String,
        text: String,
        #[serde(default)]
        timestamp_ms: u64,
        #[serde(default)]
        meeting_id: String,
    },
    /// The meeting is over.
    #[serde(rename_all = "camelCase")]
    Ended { meeting_id: String },
}

/// Which audio track a chunk belongs to.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum AudioTrack {
    /// The owner's own microphone.
    Microphone,
    /// Every remote participant, mixed.
    Remote,
}

/// Converts one extension message into a bridge frame.
///
/// Inputs: The JSON value read from the browser.
/// Outputs: The frame, or `Invalid` for an unknown type, missing fields, invalid base64, empty
/// audio, or microphone audio without a participant.
/// Side effects: None.
pub fn to_bridge_frame(message: serde_json::Value) -> Result<BridgeFrame, ExtensionMessageError> {
    let message: ExtensionMessage =
        serde_json::from_value(message).map_err(|_| invalid("unknown type or missing fields"))?;
    let mut envelope = TeamsBridgeMessage::new();
    let mut audio_duration_ms = None;
    match message {
        ExtensionMessage::Meeting {
            meeting_id,
            title,
            start_time_ms,
            participants,
        } => {
            let mut metadata = MeetingMetadata::new();
            metadata.meeting_id = meeting_id;
            metadata.meeting_title = title;
            metadata.start_time_ms = start_time_ms;
            metadata.participants = participants.into_iter().map(participant).collect();
            envelope.set_metadata(metadata);
        }
        ExtensionMessage::Audio {
            track,
            participant: sender,
            timestamp_ms,
            pcm,
        } => {
            let audio_data = decode_base64(&pcm)?;
            if audio_data.is_empty() {
                return Err(invalid("audio is empty"));
            }
            let mut audio = AudioChunk::new();
            let (track_kind, sender) = match (track, sender) {
                (AudioTrack::Microphone, Some(sender)) => (TrackKind::PARTICIPANT, sender),
                (AudioTrack::Microphone, None) => {
                    return Err(invalid("microphone audio needs its participant"));
                }
                (AudioTrack::Remote, _) => (
                    TrackKind::MIXED_REMOTE,
                    ParticipantJson {
                        id: REMOTE_MIX_PARTICIPANT_ID.to_owned(),
                        name: REMOTE_MIX_PARTICIPANT_NAME.to_owned(),
                    },
                ),
            };
            audio_duration_ms = Some(audio_data.len() as u64 / PCM_BYTES_PER_MS);
            audio.participant = protobuf::MessageField::some(participant(sender));
            audio.audio_data = audio_data;
            audio.timestamp_ms = timestamp_ms;
            audio.sample_rate = SAMPLE_RATE_HZ;
            audio.track = track_kind.into();
            envelope.set_audio(audio);
        }
        ExtensionMessage::Slide {
            timestamp_ms,
            image,
        } => {
            let mut video = VideoFrame::new();
            video.image_data = decode_base64(&image)?;
            video.timestamp_ms = timestamp_ms;
            video.source = "screen_share".to_owned();
            envelope.set_video(video);
        }
        ExtensionMessage::Document {
            kind,
            media_type,
            text,
            timestamp_ms,
            meeting_id,
        } => {
            let mut document = MeetingDocument::new();
            document.kind = kind;
            document.media_type = media_type;
            document.content = text.into_bytes();
            document.timestamp_ms = timestamp_ms;
            document.meeting_id = meeting_id;
            envelope.set_document(document);
        }
        ExtensionMessage::Ended { meeting_id } => {
            let mut ended = MeetingEnded::new();
            ended.meeting_id = meeting_id;
            envelope.set_ended(ended);
        }
    }
    let bytes = envelope
        .write_to_bytes()
        .map_err(|_| invalid("frame could not be encoded"))?;
    Ok(BridgeFrame {
        bytes,
        audio_duration_ms,
    })
}

/// Builds a bridge participant (no e-mail: the extension never sends one).
fn participant(participant_json: ParticipantJson) -> ExternalParticipant {
    let mut participant = ExternalParticipant::new();
    participant.external_id = participant_json.id;
    participant.display_name = participant_json.name;
    participant
}

/// Decodes standard base64.
fn decode_base64(encoded: &str) -> Result<Vec<u8>, ExtensionMessageError> {
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid("invalid base64"))
}

/// Builds an `Invalid` error.
fn invalid(reason: &str) -> ExtensionMessageError {
    ExtensionMessageError::Invalid {
        reason: reason.to_owned(),
    }
}
