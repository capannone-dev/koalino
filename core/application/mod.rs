//! Exposes use cases that coordinate domain work solely through ports.

/// Defines portable provider and fallback configuration for AI workloads.
pub mod ai_config;
/// Selects registered adapter-neutral AI engines from provider configuration.
pub mod ai_registry;
/// Encodes the sealed `NTA1` messages listing the attachments a device holds (Issue #57).
pub mod attachment_exchange;
#[cfg(test)]
mod attachment_exchange_tests;
/// Suggests meeting ownership for externally acquired files through metadata and storage ports.
pub mod attachment_manager;
#[cfg(test)]
mod attachment_manager_tests;
/// Decides which attachments to fetch from paired devices and drives each transfer (Issue #57).
pub mod attachment_sync;
#[cfg(test)]
mod attachment_sync_tests;
/// Lists unknown-speaker clusters and assigns them to named identities for driving adapters.
pub mod cluster_review;
#[cfg(test)]
mod cluster_review_tests;
/// Encodes the sealed `NTC1` messages that copy unknown-speaker clusters to paired devices
/// (Issue #50).
pub mod cluster_share_exchange;
#[cfg(test)]
mod cluster_share_exchange_tests;
/// Folds devices' recordings of one meeting and plans what happens to their speech (#88).
pub mod combined_meetings;
#[cfg(test)]
mod combined_meetings_tests;
/// Transcribes one meeting recorded on several devices from all of them (#88).
pub mod combined_refinement;
#[cfg(test)]
mod combined_refinement_tests;
/// Defines compute-routing policy seams for future distributed AI selection.
pub mod compute_router;
#[cfg(test)]
mod compute_router_tests;
/// Encrypts voice embeddings at rest and resolves the key protecting them.
pub mod crypto;
/// Deletes meetings and unknown voices for the CLI and the HTTP API.
pub mod deletion;
/// Encodes the `NTD1` messages that carry deletion records to paired devices.
pub mod deletion_exchange;
#[cfg(test)]
mod deletion_exchange_tests;
#[cfg(test)]
mod deletion_tests;
/// Encodes and validates the `DeviceManifest` messages paired devices exchange (Issue #43).
pub mod device_manifest_exchange;
#[cfg(test)]
mod device_manifest_exchange_tests;
/// Coordinates diarization, identity matching, and manual-review clip preservation through ports.
pub mod diarization_processor;
#[cfg(test)]
mod diarization_processor_tests;
/// Drops the microphone's copies of the computer's sound from a timeline (#108).
pub mod echo_removal;
#[cfg(test)]
mod echo_removal_tests;
/// Orchestrates repository extraction, attachment staging, and dispatch to meeting exporters.
pub mod export_manager;
#[cfg(test)]
mod export_manager_tests;
/// Transcribes with the owner's glossary of names and terms, around any engine.
pub mod glossary_transcription;
#[cfg(test)]
mod glossary_transcription_tests;
/// The daemon's runtime log level and log location, as a port for the HTTP routes (Issue #46).
pub mod log_control;
/// Coordinates Automerge meeting documents through storage ports.
pub mod meeting_crdt;
/// Implements the meeting-document schema: origin, transcript, and segments (Issue #43).
pub mod meeting_document;
#[cfg(test)]
mod meeting_document_tests;
/// The owner's edits to a meeting's notes, summary and transcript segments (#96).
pub mod meeting_edits;
#[cfg(test)]
mod meeting_edits_tests;
/// Hands a meeting to the owner's own AI app (Claude, ChatGPT) and keeps the answer (#114).
pub mod meeting_handoff;
#[cfg(test)]
mod meeting_handoff_tests;
/// Coordinates recording transcription and transcript persistence.
pub mod meeting_recorder;
/// Renames a meeting after validating the new title, for the CLI, HTTP, and FFI adapters.
pub mod meeting_rename;
#[cfg(test)]
mod meeting_rename_tests;
/// Asks a language model for a meeting's summary, condensing long transcripts in parts (#95).
pub mod meeting_summary;
/// Keeps a meeting's summary and its own prompt in the meeting document (#95).
pub mod meeting_summary_document;
#[cfg(test)]
mod meeting_summary_document_tests;
#[cfg(test)]
mod meeting_summary_tests;
#[cfg(test)]
mod meeting_title_sync_tests;
/// Builds the readable transcript text (segments, or the flat imported text) for read paths.
pub mod meeting_transcript;
#[cfg(test)]
mod meeting_transcript_tests;
/// Applies adapter-neutral encrypted-network events to application state.
pub mod network_events;
#[cfg(test)]
mod network_events_tests;
/// Asks a language model about one meeting, one project or everything (#97).
pub mod notes_chat;
#[cfg(test)]
mod notes_chat_tests;
/// Combines pairing and repository sync on one transport for the daemon's P2P task.
pub mod p2p_service;
#[cfg(test)]
mod p2p_service_tests;
/// Serializable P2P status and pairing views shared by the HTTP routes and the FFI.
pub mod p2p_view;
/// Frames the X25519 public key paired devices exchange to agree their pair key (Issue #50).
pub mod pair_key_exchange;
#[cfg(test)]
mod pair_key_exchange_tests;
/// Encodes the sealed `NTQ1`/`NTQ2` messages that ask a paired computer to answer a chat (#107).
pub mod paired_chat_exchange;
#[cfg(test)]
mod paired_chat_exchange_tests;
/// Sends a chat question to the paired computers in turn until one answers (#107).
pub mod paired_chat_relay;
#[cfg(test)]
mod paired_chat_relay_tests;
/// Orchestrates numeric-comparison pairing and the trusted-peer allowlist (Issue #43).
pub mod pairing_manager;
#[cfg(test)]
mod pairing_manager_tests;
/// Logs the transcript before and after the after-meeting pass at `trace` (#87).
pub mod pass_comparison;
#[cfg(test)]
mod pass_comparison_tests;
/// Connects transcription, diarization, and voice identity into speaker-attributed segments.
pub mod pipeline;
/// Encodes the sealed `NTP1` messages that copy the project list to paired devices (Issue #85).
pub mod project_library_exchange;
#[cfg(test)]
mod project_library_exchange_tests;
/// Runs meeting pipeline sessions on a dedicated worker thread for live recordings.
pub mod recording_manager;
#[cfg(test)]
mod recording_manager_tests;
/// Synchronizes every meeting of the repository with paired devices (Issue #43).
pub mod repository_sync_manager;
#[cfg(test)]
mod repository_sync_manager_tests;
/// Recognises two devices' recordings of one meeting from their transcripts (#88).
pub mod same_meeting;
#[cfg(test)]
mod same_meeting_tests;
/// Samples the shared screen while recording and stores new slides on the meeting (Issue #49).
pub mod screen_share_monitor;
#[cfg(test)]
mod screen_share_monitor_tests;
/// Searches meetings and projects for the owner's words (#100).
pub mod search;
#[cfg(test)]
mod search_tests;
/// Decides which screen frames are new slides (Issue #49).
pub mod slide_change_detector;
#[cfg(test)]
mod slide_change_detector_tests;
/// Encodes the sealed messages that carry a meeting's speech to the combining device (#88).
pub mod speech_transfer_exchange;
#[cfg(test)]
mod speech_transfer_exchange_tests;
/// Coordinates Automerge sync messages through encrypted-network and storage ports.
pub mod sync_manager;
#[cfg(test)]
mod sync_manager_tests;
/// Coordinates external meeting bridge media with local identities and AI ports.
pub mod teams_processor;
#[cfg(test)]
mod teams_processor_tests;
/// Defines telemetry requirements before dispatcher and storage implementation.
#[cfg(test)]
mod telemetry_tests;
/// Re-transcribes a finished meeting in long windows and keeps its speakers (#87).
pub mod transcript_refinement;
#[cfg(test)]
mod transcript_refinement_tests;
/// Encodes the sealed `NTT3`/`NTT2` messages that transcribe on a paired device (Issue #59).
pub mod transcription_exchange;
#[cfg(test)]
mod transcription_exchange_tests;
/// Routes each chunk to this device or a paired one, falling back to local (Issue #59).
pub mod transcription_routing;
#[cfg(test)]
mod transcription_routing_tests;
/// Groups unknown voices within a meeting by cosine similarity and resolves clusters to identities.
pub mod unknown_speaker_clusterer;
#[cfg(test)]
mod unknown_speaker_clusterer_tests;
/// Classifies voice embeddings and records user-confirmed signature variants.
pub mod voice_identity_manager;
#[cfg(test)]
mod voice_identity_manager_tests;
/// Encodes the sealed `NTV1` messages that copy the voice library to paired devices (Issue #56).
pub mod voice_library_exchange;
#[cfg(test)]
mod voice_library_exchange_tests;
