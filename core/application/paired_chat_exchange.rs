//! Encodes the messages that ask a paired computer to answer a chat with notes (#107).
//!
//! A phone without a language model asks a paired computer: it sends the question, the scope
//! (a meeting, a project or everything) and the recent turns of its own chat as `NTQ1`; the
//! computer gathers the material from its own synced notes and answers with `NTQ2` messages: first
//! `Accepted` (it is working on it), then `Answered` or `Declined` with an error code. Each body
//! is the job ID followed by JSON, sealed with the pair key and bound to the sender and receiver,
//! so questions, history and answers are never readable on the wire, and they are never logged.
//! The sender holds the pair key but is still peer input, so decoding bounds every value.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    application::crypto::pair_key::{open_from_peer, seal_for_peer},
    domain::{
        chat::{ChatRole, ChatScope, ChatSource, MAX_CHAT_QUESTION_CHARS},
        models::DeviceId,
        peer_trust::PairKey,
    },
};

/// Marks a request.
const REQUEST_HEADER: &[u8; 4] = b"NTQ1";
/// Marks a response.
const RESPONSE_HEADER: &[u8; 4] = b"NTQ2";
/// The largest body accepted, in bytes.
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// The job ID's length.
const JOB_ID_BYTES: usize = 16;
/// Turns of the chat so far sent with a question, at most (the latest).
pub const MAX_HISTORY_TURNS: usize = 40;
/// The longest turn or answer accepted, in bytes.
const MAX_TEXT_BYTES: usize = 256 * 1024;
/// Most `included` / `omitted` labels and sources an answer may list.
const MAX_LIST_ENTRIES: usize = 200;
/// The longest label accepted, in bytes.
const MAX_LABEL_BYTES: usize = 1024;
/// The longest error code accepted.
const MAX_ERROR_CODE_BYTES: usize = 64;

/// Why a chat message could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PairedChatError {
    /// The question is blank or too long, or the message would be too large.
    #[error("chat message is invalid or too large")]
    Invalid,
    /// The message or its body cannot be parsed, or holds implausible values.
    #[error("chat message is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("chat message failed authentication")]
    Unauthenticated,
    /// Encryption failed.
    #[error("chat message could not be encrypted")]
    EncryptionFailed,
}

/// One turn of the chat so far.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedChatTurn {
    /// Who wrote it.
    pub role: ChatRole,
    /// What it said.
    pub text: String,
}

/// A question for a paired computer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedChatRequest {
    /// Identifies the question on the asking device.
    pub job_id: Uuid,
    /// What the chat is about.
    pub scope: ChatScope,
    /// The question.
    pub question: String,
    /// The chat so far, oldest first (only the latest `MAX_HISTORY_TURNS` are sent).
    pub history: Vec<PairedChatTurn>,
}

/// A computer's answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedChatAnswer {
    /// The answer.
    pub text: String,
    /// The material given to the model.
    pub included: Vec<String>,
    /// The material left out for length.
    pub omitted: Vec<String>,
    /// The places the answer cites.
    pub sources: Vec<ChatSource>,
    /// The connection that answered, such as `Claude Code on Laptop`.
    pub connection_label: String,
    /// The answer stopped at the length limit.
    pub truncated: bool,
    /// Images the model was sent.
    pub images_sent: u32,
}

/// Where a question stands on the computer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairedChatOutcome {
    /// The computer is working on it.
    Accepted,
    /// The answer.
    Answered(PairedChatAnswer),
    /// No answer, with the reason as an error code such as `not_offered` or `llm_unreachable`.
    Declined {
        /// Lowercase letters, digits and underscores.
        error_code: String,
    },
}

/// A response to one question.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedChatResponse {
    /// The question's ID on the asking device.
    pub job_id: Uuid,
    /// Where it stands.
    pub outcome: PairedChatOutcome,
}

/// The request's JSON body.
#[derive(Serialize, Deserialize)]
struct RequestBody {
    scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    question: String,
    history: Vec<TurnBody>,
}

/// One turn's JSON.
#[derive(Serialize, Deserialize)]
struct TurnBody {
    role: String,
    text: String,
}

/// The response's JSON body.
#[derive(Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum ResponseBody {
    Accepted,
    Answered {
        text: String,
        included: Vec<String>,
        omitted: Vec<String>,
        sources: Vec<ChatSource>,
        connection: String,
        truncated: bool,
        images_sent: u32,
    },
    Declined {
        error_code: String,
    },
}

/// Reports whether `payload` is a chat request. Side effects: None.
pub fn is_paired_chat_request(payload: &[u8]) -> bool {
    payload.starts_with(REQUEST_HEADER)
}

/// Reports whether `payload` is a chat response. Side effects: None.
pub fn is_paired_chat_response(payload: &[u8]) -> bool {
    payload.starts_with(RESPONSE_HEADER)
}

/// Seals a question.
///
/// Inputs: The request, the pair key, and the two device IDs it is bound to.
/// Outputs: The message, or `Invalid` for a blank or overlong question or a body that is too
/// large. Only the latest `MAX_HISTORY_TURNS` turns are sent. Side effects: None.
pub fn encode_paired_chat_request(
    request: &PairedChatRequest,
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, PairedChatError> {
    let question = request.question.trim();
    if question.is_empty() || question.chars().count() > MAX_CHAT_QUESTION_CHARS {
        return Err(PairedChatError::Invalid);
    }
    let (scope, id) = scope_parts(&request.scope);
    let skipped = request.history.len().saturating_sub(MAX_HISTORY_TURNS);
    let body = RequestBody {
        scope: scope.to_owned(),
        id,
        question: question.to_owned(),
        history: request.history[skipped..]
            .iter()
            .map(|turn| TurnBody {
                role: turn.role.as_str().to_owned(),
                text: turn.text.clone(),
            })
            .collect(),
    };
    seal(REQUEST_HEADER, request.job_id, &body, pair_key, from, to)
}

/// Opens and validates a question.
///
/// Outputs: The request, or why it was refused. Side effects: None.
pub fn decode_paired_chat_request(
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<PairedChatRequest, PairedChatError> {
    let (job_id, body): (Uuid, RequestBody) = open(REQUEST_HEADER, message, pair_key, from, to)?;
    let scope =
        ChatScope::parse(&body.scope, body.id.as_deref()).ok_or(PairedChatError::Malformed)?;
    let question = body.question.trim();
    if question.is_empty()
        || question.chars().count() > MAX_CHAT_QUESTION_CHARS
        || body.history.len() > MAX_HISTORY_TURNS
    {
        return Err(PairedChatError::Malformed);
    }
    let history = body
        .history
        .into_iter()
        .map(|turn| {
            let role = ChatRole::parse(&turn.role).ok_or(PairedChatError::Malformed)?;
            if turn.text.len() > MAX_TEXT_BYTES {
                return Err(PairedChatError::Malformed);
            }
            Ok(PairedChatTurn {
                role,
                text: turn.text,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(PairedChatRequest {
        job_id,
        scope,
        question: question.to_owned(),
        history,
    })
}

/// Seals a response.
///
/// Outputs: The message, or `Invalid` for a body that is too large. Side effects: None.
pub fn encode_paired_chat_response(
    response: &PairedChatResponse,
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, PairedChatError> {
    let body = match &response.outcome {
        PairedChatOutcome::Accepted => ResponseBody::Accepted,
        PairedChatOutcome::Answered(answer) => ResponseBody::Answered {
            text: answer.text.clone(),
            included: answer.included.clone(),
            omitted: answer.omitted.clone(),
            sources: answer.sources.clone(),
            connection: answer.connection_label.clone(),
            truncated: answer.truncated,
            images_sent: answer.images_sent,
        },
        PairedChatOutcome::Declined { error_code } => ResponseBody::Declined {
            error_code: error_code.clone(),
        },
    };
    seal(RESPONSE_HEADER, response.job_id, &body, pair_key, from, to)
}

/// Opens and validates a response.
///
/// Outputs: The response, or why it was refused. Side effects: None.
pub fn decode_paired_chat_response(
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<PairedChatResponse, PairedChatError> {
    let (job_id, body): (Uuid, ResponseBody) = open(RESPONSE_HEADER, message, pair_key, from, to)?;
    let outcome = match body {
        ResponseBody::Accepted => PairedChatOutcome::Accepted,
        ResponseBody::Declined { error_code } => {
            let plausible = !error_code.is_empty()
                && error_code.len() <= MAX_ERROR_CODE_BYTES
                && error_code
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
            if !plausible {
                return Err(PairedChatError::Malformed);
            }
            PairedChatOutcome::Declined { error_code }
        }
        ResponseBody::Answered {
            text,
            included,
            omitted,
            sources,
            connection,
            truncated,
            images_sent,
        } => {
            let labels_fit = |labels: &[String]| {
                labels.len() <= MAX_LIST_ENTRIES
                    && labels.iter().all(|label| label.len() <= MAX_LABEL_BYTES)
            };
            let plausible = text.len() <= MAX_TEXT_BYTES
                && labels_fit(&included)
                && labels_fit(&omitted)
                && sources.len() <= MAX_LIST_ENTRIES
                && sources
                    .iter()
                    .all(|source| source.label.len() <= MAX_LABEL_BYTES)
                && connection.len() <= MAX_LABEL_BYTES;
            if !plausible {
                return Err(PairedChatError::Malformed);
            }
            PairedChatOutcome::Answered(PairedChatAnswer {
                text,
                included,
                omitted,
                sources,
                connection_label: connection,
                truncated,
                images_sent,
            })
        }
    };
    Ok(PairedChatResponse { job_id, outcome })
}

/// A scope's kind and ID, as `ChatScope::parse` reads them.
fn scope_parts(scope: &ChatScope) -> (&'static str, Option<String>) {
    match scope {
        ChatScope::Meeting(meeting_id) => ("meeting", Some(meeting_id.as_str().to_owned())),
        ChatScope::Project(project_id) => ("project", Some(project_id.to_string())),
        ChatScope::All => ("all", None),
    }
}

/// Seals the job ID and `body` as JSON under `header`, which is also the sealing purpose.
fn seal(
    header: &[u8; 4],
    job_id: Uuid,
    body: &impl Serialize,
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, PairedChatError> {
    let json = Zeroizing::new(serde_json::to_vec(body).map_err(|_| PairedChatError::Invalid)?);
    if json.len() > MAX_BODY_BYTES {
        return Err(PairedChatError::Invalid);
    }
    let mut plain = Zeroizing::new(Vec::with_capacity(JOB_ID_BYTES + json.len()));
    plain.extend_from_slice(job_id.as_bytes());
    plain.extend_from_slice(&json);
    let sealed = seal_for_peer(pair_key, header, from, to, &plain)
        .map_err(|_| PairedChatError::EncryptionFailed)?;
    let mut message = header.to_vec();
    message.extend_from_slice(&sealed);
    Ok(message)
}

/// Opens a message sealed by `seal` and reads its JSON body.
fn open<Body: for<'de> Deserialize<'de>>(
    header: &[u8; 4],
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<(Uuid, Body), PairedChatError> {
    let sealed = message
        .strip_prefix(header)
        .ok_or(PairedChatError::Malformed)?;
    let plain = open_from_peer(pair_key, header, from, to, sealed)
        .map_err(|_| PairedChatError::Unauthenticated)?;
    let (job_id, json) = plain
        .split_first_chunk::<JOB_ID_BYTES>()
        .ok_or(PairedChatError::Malformed)?;
    if json.len() > MAX_BODY_BYTES {
        return Err(PairedChatError::Malformed);
    }
    let body = serde_json::from_slice(json).map_err(|_| PairedChatError::Malformed)?;
    Ok((Uuid::from_bytes(*job_id), body))
}
