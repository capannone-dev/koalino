//! Exposes pure domain models and errors for the meeting-note business language.

/// Notices a call from the apps using the microphone (#111).
pub mod call_watch;
#[cfg(test)]
mod call_watch_tests;
/// What a chat with notes is about, and its messages (#97).
pub mod chat;
#[cfg(test)]
mod chat_tests;
/// Hears whether the other side's voice reaches the microphone (#108).
pub mod echo_detect;
#[cfg(test)]
mod echo_detect_tests;
/// Finds microphone lines that only repeat the computer's sound (#108).
pub mod echo_lines;
#[cfg(test)]
mod echo_lines_tests;
/// Describes an imported e-mail and its attachments (Issue #85).
pub mod email;
/// Defines failures that may cross a hexagonal boundary.
pub mod errors;
/// Defines LLM connections, the endpoint rule and the summary prompt template (#95).
pub mod llm;
#[cfg(test)]
mod llm_tests;
/// Defines typed meeting-note business concepts.
pub mod models;
/// Defines paired devices and the numeric-comparison pairing code (Issue #43).
pub mod network_addresses;
#[cfg(test)]
mod network_addresses_tests;
/// Judges whether an online-meeting participant's track carries one voice or several (#103).
pub mod participant_track_voices;
#[cfg(test)]
mod participant_track_voices_tests;
/// Wraps bridge PCM in a WAV header for the speaker-separation port (#102).
pub mod pcm_wav;
#[cfg(test)]
mod pcm_wav_tests;
pub mod peer_trust;
#[cfg(test)]
mod peer_trust_tests;
/// Defines projects: folders of meetings and external resources (Issue #85).
pub mod projects;
#[cfg(test)]
mod projects_tests;
/// Judges how far a transcript can be trusted: uncertain words, overlap and speakers.
pub mod transcript_confidence;
#[cfg(test)]
mod transcript_confidence_tests;
/// Defines the glossary of names and terms that steers transcription.
pub mod transcription_glossary;
#[cfg(test)]
mod transcription_glossary_tests;
/// Defines the language a recording is transcribed in.
pub mod transcription_language;
#[cfg(test)]
mod transcription_language_tests;
/// Lists the downloadable Whisper models with their pinned hashes.
pub mod whisper_catalog;
#[cfg(test)]
mod whisper_catalog_tests;
/// Proposes a different Whisper model from how the current one runs on this device.
pub mod whisper_proposal;
#[cfg(test)]
mod whisper_proposal_tests;
/// Times a short clip on this device to confirm Automatic's model (#109).
pub mod whisper_speed_check;
#[cfg(test)]
mod whisper_speed_check_tests;
/// Suggests a Whisper model for a device from what it can run.
pub mod whisper_suggestion;
#[cfg(test)]
mod whisper_suggestion_tests;
