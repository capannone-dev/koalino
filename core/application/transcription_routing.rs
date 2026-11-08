//! Sends transcription to where the owner chose: this device, a paired device, or the best one
//! (Issue #59).
//!
//! `RoutedTranscriptionEngine` is the `TranscriptionEngine` the pipeline uses. For each chunk it
//! registers the connected devices' manifests with a `ComputeDispatcher`, which picks the target.
//! A chunk's samples are only copied for a remote attempt and deleted once it succeeds, so any
//! failure (disconnection, timeout, a busy or refusing device) falls back to the local engine with
//! the audio intact. Audio and text are never logged.

use crate::domain::transcription_language::TranscriptionLanguage;
use std::{cell::Cell, rc::Rc, sync::Arc};

use crate::{
    application::compute_router::{ComputeDispatcher, ComputePreference},
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, DeviceManifest, ExternalMediaChunk, Transcript,
            TranscriptionContext, TranscriptionModel,
        },
    },
    ports::{ai::TranscriptionEngine, remote_transcription::ChunkSamples},
};

/// The text form of "this device only".
const LOCAL_ROUTE: &str = "local";
/// The text form of "the best connected device".
const AUTO_ROUTE: &str = "auto";

/// Writes a route as stored and shown: `local`, `auto`, or a device's peer ID.
///
/// Inputs: The route. Outputs: Its text. Side effects: None.
pub fn route_text(route: &ComputePreference) -> String {
    match route {
        ComputePreference::LocalOnly => LOCAL_ROUTE.to_owned(),
        ComputePreference::Automatic => AUTO_ROUTE.to_owned(),
        ComputePreference::SpecificPeer(device_id) => device_id.as_str().to_owned(),
    }
}

/// Reads a route's text; anything but `local` and `auto` names a device.
///
/// Inputs: Non-empty text. Outputs: The route. Side effects: None.
pub fn parse_route(text: &str) -> ComputePreference {
    match text {
        LOCAL_ROUTE => ComputePreference::LocalOnly,
        AUTO_ROUTE => ComputePreference::Automatic,
        device_id => ComputePreference::SpecificPeer(DeviceId::new(device_id)),
    }
}

/// How the pipeline reaches the P2P task.
pub trait RemoteTranscriptionGateway: Send + Sync {
    /// The owner's current choice. Side effects: None.
    fn route(&self) -> ComputePreference;

    /// This device's manifest. Side effects: None.
    fn local_manifest(&self) -> DeviceManifest;

    /// The manifests of connected paired devices. Side effects: None.
    fn connected_manifests(&self) -> Vec<DeviceManifest>;

    /// Asks `peer` to transcribe and waits for the answer.
    ///
    /// Inputs: 16 kHz mono samples, the previous chunk's text, and the language chosen for the
    /// chunk (`None` leaves it to the other device's configuration).
    /// Outputs: The text, or `Unavailable` on disconnection, timeout or refusal.
    /// Side effects: Sends the audio sealed to `peer`, wipes this copy, and blocks the caller, so
    /// it must run on the pipeline worker, never on a UI thread or an async runtime.
    fn transcribe_on_peer(
        &self,
        peer: &DeviceId,
        samples: Vec<i16>,
        previous_text: Option<String>,
        language: Option<TranscriptionLanguage>,
    ) -> Result<String, TranscriptionError>;
}

/// The dispatcher's view of the local engine; records that it ran.
struct LocalTarget<LocalEngine> {
    engine: Option<Rc<LocalEngine>>,
    invoked: Rc<Cell<bool>>,
}

impl<LocalEngine: TranscriptionEngine> TranscriptionEngine for LocalTarget<LocalEngine> {
    /// Runs the local engine once.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.invoked.set(true);
        match &self.engine {
            Some(engine) => engine.transcribe(audio_recording, transcription_context),
            None => Err(TranscriptionError::Unavailable),
        }
    }
}

/// The dispatcher's view of one paired device.
struct RemoteTarget {
    peer: DeviceId,
    gateway: Arc<dyn RemoteTranscriptionGateway>,
    chunk_samples: Arc<dyn ChunkSamples>,
}

impl TranscriptionEngine for RemoteTarget {
    /// Transcribes on the device, deleting the local audio only after it succeeded.
    ///
    /// Side effects: Blocks on the network; see `transcribe_on_peer`.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        let samples = self
            .chunk_samples
            .copy_samples(audio_recording.recording_id())
            .ok_or(TranscriptionError::InvalidRecording {
                reason: String::from("the chunk's audio is not available"),
            })?;
        let text = self.gateway.transcribe_on_peer(
            &self.peer,
            samples,
            // The host prompts its engine with this text, so the glossary reaches it too.
            transcription_context.prompt(),
            transcription_context.language(),
        )?;
        self.chunk_samples
            .delete_samples(audio_recording.recording_id());
        Ok(Transcript::new(audio_recording.meeting_id().clone(), text))
    }
}

/// Routes each chunk; the pipeline's transcription engine.
pub struct RoutedTranscriptionEngine<LocalEngine> {
    local_engine: Option<Rc<LocalEngine>>,
    remote: Option<(Arc<dyn RemoteTranscriptionGateway>, Arc<dyn ChunkSamples>)>,
}

impl<LocalEngine: TranscriptionEngine + 'static> RoutedTranscriptionEngine<LocalEngine> {
    /// Creates an engine that always runs locally: P2P is off.
    ///
    /// Inputs: The local engine. Outputs: The engine. Side effects: None.
    pub fn local(local_engine: LocalEngine) -> Self {
        Self {
            local_engine: Some(Rc::new(local_engine)),
            remote: None,
        }
    }

    /// Creates an engine that follows the owner's choice.
    ///
    /// Inputs: The local engine (`None` on a device that can only use others), the gateway, and
    /// the chunks' samples. Outputs: The engine. Side effects: None.
    pub fn routed(
        local_engine: Option<LocalEngine>,
        gateway: Arc<dyn RemoteTranscriptionGateway>,
        chunk_samples: Arc<dyn ChunkSamples>,
    ) -> Self {
        Self {
            local_engine: local_engine.map(Rc::new),
            remote: Some((gateway, chunk_samples)),
        }
    }

    /// Runs the local engine, or reports that this device has none.
    fn transcribe_locally(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        match &self.local_engine {
            Some(engine) => engine.transcribe(audio_recording, transcription_context),
            None => Err(TranscriptionError::Unavailable),
        }
    }
}

impl<LocalEngine: TranscriptionEngine + 'static> TranscriptionEngine
    for RoutedTranscriptionEngine<LocalEngine>
{
    /// Transcribes one chunk where the owner chose, falling back to the local engine.
    ///
    /// Inputs: The chunk and the previous chunk's text.
    /// Outputs: The transcript, or the local engine's error (`Unavailable` without one).
    /// Side effects: May send the audio sealed to a paired device and wait for it. The audio is
    /// deleted here after a remote success, or by the local engine as it reads it.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        let Some((gateway, chunk_samples)) = &self.remote else {
            return self.transcribe_locally(audio_recording, transcription_context);
        };
        let route = gateway.route();
        if route == ComputePreference::LocalOnly {
            return self.transcribe_locally(audio_recording, transcription_context);
        }
        let manifests = gateway.connected_manifests();
        let local_manifest = gateway.local_manifest();
        // A device with no local engine must not be picked by Auto, whatever it advertises.
        let local_manifest = if self.local_engine.is_some() {
            local_manifest
        } else {
            DeviceManifest::new(local_manifest.device_id().clone(), 0, 0, Vec::new())
        };
        let model = requested_model(&route, &local_manifest, &manifests);
        let local_invoked = Rc::new(Cell::new(false));
        let mut dispatcher = ComputeDispatcher::new(
            local_manifest,
            LocalTarget {
                engine: self.local_engine.clone(),
                invoked: Rc::clone(&local_invoked),
            },
        );
        for manifest in manifests {
            let peer = manifest.device_id().clone();
            dispatcher.register_remote_engine(
                manifest,
                RemoteTarget {
                    peer,
                    gateway: Arc::clone(gateway),
                    chunk_samples: Arc::clone(chunk_samples),
                },
            );
        }
        match dispatcher.dispatch_with_context(
            audio_recording,
            model,
            &route,
            transcription_context,
        ) {
            Ok(transcript) => Ok(transcript),
            Err(error) => {
                tracing::warn!(error = %error, "routed transcription failed");
                if local_invoked.get() {
                    return Err(TranscriptionError::Unavailable);
                }
                self.transcribe_locally(audio_recording, transcription_context)
            }
        }
    }

    /// Transcribes bridge audio locally: this path is not routed.
    ///
    /// Outputs: The transcript, or `Unavailable` without a local engine.
    /// Side effects: Those of the local engine.
    fn transcribe_external_media(
        &self,
        media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        match &self.local_engine {
            Some(engine) => engine.transcribe_external_media(media_chunk, audio_recording),
            None => Err(TranscriptionError::Unavailable),
        }
    }
}

/// Picks the model to ask for: what the chosen device offers, otherwise what this device runs.
///
/// Inputs: The route and the manifests. Outputs: The model, Whisper when nothing says otherwise.
/// Side effects: None.
fn requested_model(
    route: &ComputePreference,
    local_manifest: &DeviceManifest,
    manifests: &[DeviceManifest],
) -> TranscriptionModel {
    let chosen = match route {
        ComputePreference::SpecificPeer(device_id) => manifests
            .iter()
            .find(|manifest| manifest.device_id() == device_id),
        _ => Some(local_manifest),
    };
    chosen
        .and_then(|manifest| manifest.available_transcription_models().first())
        .cloned()
        .unwrap_or(TranscriptionModel::Whisper)
}
