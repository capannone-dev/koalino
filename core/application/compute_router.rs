//! Routes transcription work and records privacy-safe execution telemetry through injected ports.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            AudioRecording, DeviceId, DeviceManifest, MeetingId, TaskMetrics, TaskType, Transcript,
            TranscriptionContext, TranscriptionModel,
        },
    },
    ports::{ai::TranscriptionEngine, telemetry::MetricsRepository},
};

/// Captures the user-selected policy for an AI workload without selecting an adapter directly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputePreference {
    /// Requires processing to run on the capturing device.
    LocalOnly,
    /// Requires processing on a user-selected, authenticated peer.
    SpecificPeer(DeviceId),
    /// Permits a future policy implementation to select the best capable peer.
    Automatic,
}

/// Reports a routing decision that cannot be satisfied by registered device manifests.
#[derive(Debug, thiserror::Error)]
pub enum ComputeDispatchError {
    /// The requested remote device has not registered a transcription port with the dispatcher.
    #[error("requested peer is unavailable: {device_id}")]
    PeerUnavailable {
        /// The remote device selected by the user.
        device_id: String,
    },
    /// A selected device does not advertise the requested transcription model.
    #[error("device {device_id} does not support the requested transcription model")]
    ModelUnavailable {
        /// The selected device that lacks the requested model.
        device_id: String,
    },
    /// The selected trait-based engine could not produce a transcript.
    #[error(transparent)]
    Transcription(#[from] TranscriptionError),
    /// Telemetry storage could not persist a completed task metric.
    #[error(transparent)]
    Metrics(#[from] StorageError),
}

/// Owns a manifest and a transcription port for one local or registered remote compute target.
struct ComputeTarget {
    manifest: DeviceManifest,
    transcription_engine: Box<dyn TranscriptionEngine>,
}

/// Dispatches transcription requests according to user preference and advertised capacity.
///
/// This application service intentionally owns only `TranscriptionEngine` trait objects. Concrete
/// Whisper, API, and encrypted P2P adapters are composed outside this hexagonal boundary.
pub struct ComputeDispatcher {
    local_target: ComputeTarget,
    remote_targets: Vec<ComputeTarget>,
    metrics_repository: Option<Box<dyn MetricsRepository>>,
    previous_chunk_transcripts: Mutex<HashMap<MeetingId, String>>,
}

impl ComputeDispatcher {
    /// Creates a dispatcher with the local node's manifest and transcription port.
    ///
    /// Inputs: `local_manifest` describes this node; `local_transcription_engine` implements the
    /// adapter-neutral transcription port.
    /// Outputs: A dispatcher with no registered remote compute targets.
    /// Side effects: Takes ownership of the injected port; it does not probe hardware or network state.
    pub fn new<LocalTranscriptionEngine>(
        local_manifest: DeviceManifest,
        local_transcription_engine: LocalTranscriptionEngine,
    ) -> Self
    where
        LocalTranscriptionEngine: TranscriptionEngine + 'static,
    {
        Self {
            local_target: ComputeTarget {
                manifest: local_manifest,
                transcription_engine: Box::new(local_transcription_engine),
            },
            remote_targets: Vec::new(),
            metrics_repository: None,
            previous_chunk_transcripts: Mutex::new(HashMap::new()),
        }
    }

    /// Configures the optional telemetry sink used after successful AI task completion.
    ///
    /// Inputs: `metrics_repository` persists only safe aggregate execution metadata.
    /// Outputs: None.
    /// Side effects: Replaces the dispatcher telemetry sink without invoking an AI engine.
    pub fn set_metrics_repository<Repository>(&mut self, metrics_repository: Repository)
    where
        Repository: MetricsRepository + 'static,
    {
        self.metrics_repository = Some(Box::new(metrics_repository));
    }

    /// Loads metrics for one device through the configured telemetry persistence port.
    ///
    /// Inputs: `device_id` identifies a local or remote compute target.
    /// Outputs: Persisted metrics or a portable storage error; an absent sink returns an empty list.
    /// Side effects: Reads local telemetry persistence only.
    pub fn metrics_for_device(&self, device_id: &str) -> Result<Vec<TaskMetrics>, StorageError> {
        match &self.metrics_repository {
            Some(metrics_repository) => metrics_repository.get_metrics_for_device(device_id),
            None => Ok(Vec::new()),
        }
    }

    /// Registers or replaces an encrypted-peer compute target represented by its latest manifest.
    ///
    /// Inputs: `remote_manifest` identifies a peer and its advertised capability; the engine is an
    /// adapter-neutral proxy that reaches that peer through the networking layer.
    /// Outputs: None.
    /// Side effects: Replaces any existing target with the same device identifier in local memory.
    pub fn register_remote_engine<RemoteTranscriptionEngine>(
        &mut self,
        remote_manifest: DeviceManifest,
        remote_transcription_engine: RemoteTranscriptionEngine,
    ) where
        RemoteTranscriptionEngine: TranscriptionEngine + 'static,
    {
        self.remote_targets.retain(|existing_target| {
            existing_target.manifest.device_id() != remote_manifest.device_id()
        });
        self.remote_targets.push(ComputeTarget {
            manifest: remote_manifest,
            transcription_engine: Box::new(remote_transcription_engine),
        });
    }

    /// Selects a compatible port and transcribes one recording according to the user preference.
    ///
    /// Inputs: `audio_recording` identifies adapter-owned audio; `transcription_model` selects a
    /// required capability; `compute_preference` constrains the eligible target set. The previous
    /// successful transcript for the recording's meeting is forwarded as portable context.
    /// Outputs: A generated transcript or a portable routing/transcription failure.
    /// Side effects: Invokes exactly one selected transcription port, which may perform local or
    /// encrypted remote computation.
    pub fn dispatch_transcription(
        &self,
        audio_recording: &AudioRecording,
        transcription_model: TranscriptionModel,
        compute_preference: &ComputePreference,
    ) -> Result<Transcript, ComputeDispatchError> {
        let selected_target = match compute_preference {
            ComputePreference::LocalOnly => {
                self.require_model(&self.local_target, &transcription_model)?
            }
            ComputePreference::SpecificPeer(device_id) => {
                self.select_specific_peer(device_id, &transcription_model)?
            }
            ComputePreference::Automatic => self.select_automatic_target(&transcription_model)?,
        };

        let execution_started_at = Instant::now();
        let transcription_context = self.transcription_context_for(audio_recording.meeting_id())?;
        let transcript = selected_target
            .transcription_engine
            .transcribe(audio_recording, &transcription_context)
            .map_err(ComputeDispatchError::from)?;
        self.remember_transcript(&transcript)?;
        self.record_transcription_metrics(
            selected_target,
            transcription_model,
            execution_started_at,
        )?;
        Ok(transcript)
    }

    /// Builds portable context from the prior successful chunk of one meeting.
    ///
    /// Inputs: `meeting_id` scopes context to one meeting, preventing transcript text from one
    /// meeting from reaching another meeting's transcription request.
    /// Outputs: Context containing the immediately previous transcript when present, or empty
    /// context for the first chunk.
    /// Side effects: Reads the dispatcher's in-memory rolling transcript buffer.
    fn transcription_context_for(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<TranscriptionContext, ComputeDispatchError> {
        let previous_text = self
            .previous_chunk_transcripts
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?
            .get(meeting_id)
            .cloned();
        Ok(TranscriptionContext::new(previous_text))
    }

    /// Stores a successful transcript for the next chunk from the same meeting.
    ///
    /// Inputs: `transcript` was produced by the selected transcription port.
    /// Outputs: Success or a portable unavailable error if the in-memory buffer is poisoned.
    /// Side effects: Replaces the in-memory prior-chunk transcript for its meeting.
    fn remember_transcript(&self, transcript: &Transcript) -> Result<(), ComputeDispatchError> {
        self.previous_chunk_transcripts
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?
            .insert(
                transcript.meeting_id().clone(),
                transcript.content().to_owned(),
            );
        Ok(())
    }

    /// Times no AI work itself; it persists completion telemetry after the selected port returns.
    fn record_transcription_metrics(
        &self,
        selected_target: &ComputeTarget,
        transcription_model: TranscriptionModel,
        execution_started_at: Instant,
    ) -> Result<(), ComputeDispatchError> {
        let Some(metrics_repository) = &self.metrics_repository else {
            return Ok(());
        };
        let duration_ms =
            u64::try_from(execution_started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        let watt_milliwatts = if selected_target.manifest.gpu_vram_megabytes() > 0 {
            25_000.0
        } else {
            5_000.0
        };
        let estimated_power_mwh = duration_ms as f32 * watt_milliwatts / 3_600_000.0;
        let timestamp_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        metrics_repository.save_metrics(TaskMetrics::new(
            uuid::Uuid::new_v4(),
            selected_target.manifest.device_id().clone(),
            TaskType::Transcription,
            transcription_model.as_str(),
            duration_ms,
            duration_ms,
            estimated_power_mwh,
            timestamp_epoch_ms,
        ))?;
        Ok(())
    }

    /// Validates one target's advertised model capability before its port is invoked.
    fn require_model<'target>(
        &'target self,
        compute_target: &'target ComputeTarget,
        transcription_model: &TranscriptionModel,
    ) -> Result<&'target ComputeTarget, ComputeDispatchError> {
        if compute_target
            .manifest
            .supports_transcription_model(transcription_model)
        {
            Ok(compute_target)
        } else {
            Err(ComputeDispatchError::ModelUnavailable {
                device_id: compute_target.manifest.device_id().as_str().to_owned(),
            })
        }
    }

    /// Finds a user-selected remote target and verifies its advertised model support.
    fn select_specific_peer(
        &self,
        requested_device_id: &DeviceId,
        transcription_model: &TranscriptionModel,
    ) -> Result<&ComputeTarget, ComputeDispatchError> {
        let selected_target = self
            .remote_targets
            .iter()
            .find(|remote_target| remote_target.manifest.device_id() == requested_device_id)
            .ok_or_else(|| ComputeDispatchError::PeerUnavailable {
                device_id: requested_device_id.as_str().to_owned(),
            })?;

        self.require_model(selected_target, transcription_model)
    }

    /// Selects the highest-capacity compatible local or remote target for automatic routing.
    fn select_automatic_target(
        &self,
        transcription_model: &TranscriptionModel,
    ) -> Result<&ComputeTarget, ComputeDispatchError> {
        let mut compatible_targets = std::iter::once(&self.local_target)
            .chain(self.remote_targets.iter())
            .filter(|compute_target| {
                compute_target
                    .manifest
                    .supports_transcription_model(transcription_model)
            });
        let first_compatible_target =
            compatible_targets
                .next()
                .ok_or_else(|| ComputeDispatchError::ModelUnavailable {
                    device_id: "no registered device".to_owned(),
                })?;

        Ok(
            compatible_targets.fold(first_compatible_target, |best_target, candidate_target| {
                if candidate_target.manifest.transcription_capacity_score()
                    > best_target.manifest.transcription_capacity_score()
                {
                    candidate_target
                } else {
                    best_target
                }
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use crate::{
        domain::{
            errors::TranscriptionError,
            models::{
                AudioRecording, DeviceId, DeviceManifest, MeetingId, RecordingId, Transcript,
                TranscriptionContext, TranscriptionModel,
            },
        },
        ports::ai::TranscriptionEngine,
    };

    use super::{ComputeDispatchError, ComputeDispatcher, ComputePreference};

    /// Provides deterministic transcription output without loading an AI model in the test sandbox.
    struct MockTranscriptionEngine {
        device_label: &'static str,
        invocation_count: Rc<Cell<u32>>,
    }

    impl MockTranscriptionEngine {
        /// Creates a deterministic mock adapter and exposes its invocation counter to the test.
        fn new(device_label: &'static str, invocation_count: Rc<Cell<u32>>) -> Self {
            Self {
                device_label,
                invocation_count,
            }
        }
    }

    impl TranscriptionEngine for MockTranscriptionEngine {
        fn transcribe(
            &self,
            audio_recording: &AudioRecording,
            _transcription_context: &TranscriptionContext,
        ) -> Result<Transcript, TranscriptionError> {
            self.invocation_count
                .set(self.invocation_count.get().saturating_add(1));
            Ok(Transcript::new(
                audio_recording.meeting_id().clone(),
                format!("mock transcript from {}", self.device_label),
            ))
        }
    }

    /// Builds portable recording metadata for dispatcher-routing tests.
    fn test_recording() -> AudioRecording {
        AudioRecording::new(
            RecordingId::new("recording-routing-test"),
            MeetingId::new("meeting-routing-test"),
            DeviceId::new("capture-device"),
        )
    }

    /// Builds an AI-capable manifest without accessing real hardware or network state.
    fn manifest(
        device_id: &str,
        cpu_cores: u16,
        gpu_vram_megabytes: u32,
        available_models: Vec<TranscriptionModel>,
    ) -> DeviceManifest {
        DeviceManifest::new(
            DeviceId::new(device_id),
            cpu_cores,
            gpu_vram_megabytes,
            available_models,
        )
    }

    #[test]
    fn local_preference_invokes_only_the_local_transcription_port() {
        let local_calls = Rc::new(Cell::new(0));
        let remote_calls = Rc::new(Cell::new(0));
        let mut dispatcher = ComputeDispatcher::new(
            manifest("local", 2, 0, vec![TranscriptionModel::Whisper]),
            MockTranscriptionEngine::new("local", Rc::clone(&local_calls)),
        );
        dispatcher.register_remote_engine(
            manifest("remote", 32, 48_000, vec![TranscriptionModel::Whisper]),
            MockTranscriptionEngine::new("remote", Rc::clone(&remote_calls)),
        );

        let transcript = dispatcher
            .dispatch_transcription(
                &test_recording(),
                TranscriptionModel::Whisper,
                &ComputePreference::LocalOnly,
            )
            .unwrap_or_else(|error| panic!("local dispatch should succeed: {error}"));

        assert_eq!(transcript.content(), "mock transcript from local");
        assert_eq!(local_calls.get(), 1);
        assert_eq!(remote_calls.get(), 0);
    }

    #[test]
    fn specific_peer_preference_invokes_only_the_requested_eligible_port() {
        let local_calls = Rc::new(Cell::new(0));
        let remote_calls = Rc::new(Cell::new(0));
        let remote_device_id = DeviceId::new("remote");
        let mut dispatcher = ComputeDispatcher::new(
            manifest("local", 8, 0, vec![TranscriptionModel::Whisper]),
            MockTranscriptionEngine::new("local", Rc::clone(&local_calls)),
        );
        dispatcher.register_remote_engine(
            manifest("remote", 4, 2_000, vec![TranscriptionModel::Whisper]),
            MockTranscriptionEngine::new("remote", Rc::clone(&remote_calls)),
        );

        let transcript = dispatcher
            .dispatch_transcription(
                &test_recording(),
                TranscriptionModel::Whisper,
                &ComputePreference::SpecificPeer(remote_device_id),
            )
            .unwrap_or_else(|error| {
                panic!("specific eligible peer dispatch should succeed: {error}")
            });

        assert_eq!(transcript.content(), "mock transcript from remote");
        assert_eq!(local_calls.get(), 0);
        assert_eq!(remote_calls.get(), 1);
    }

    #[test]
    fn automatic_preference_selects_the_highest_capacity_eligible_manifest() {
        let local_calls = Rc::new(Cell::new(0));
        let slower_remote_calls = Rc::new(Cell::new(0));
        let faster_remote_calls = Rc::new(Cell::new(0));
        let mut dispatcher = ComputeDispatcher::new(
            manifest("local", 4, 0, vec![TranscriptionModel::Whisper]),
            MockTranscriptionEngine::new("local", Rc::clone(&local_calls)),
        );
        dispatcher.register_remote_engine(
            manifest(
                "remote-slower",
                12,
                4_000,
                vec![TranscriptionModel::Whisper],
            ),
            MockTranscriptionEngine::new("remote-slower", Rc::clone(&slower_remote_calls)),
        );
        dispatcher.register_remote_engine(
            manifest(
                "remote-faster",
                8,
                16_000,
                vec![TranscriptionModel::Whisper],
            ),
            MockTranscriptionEngine::new("remote-faster", Rc::clone(&faster_remote_calls)),
        );

        let transcript = dispatcher
            .dispatch_transcription(
                &test_recording(),
                TranscriptionModel::Whisper,
                &ComputePreference::Automatic,
            )
            .unwrap_or_else(|error| panic!("automatic dispatch should succeed: {error}"));

        assert_eq!(transcript.content(), "mock transcript from remote-faster");
        assert_eq!(local_calls.get(), 0);
        assert_eq!(slower_remote_calls.get(), 0);
        assert_eq!(faster_remote_calls.get(), 1);
    }

    #[test]
    fn automatic_preference_ignores_ineligible_remotes_and_falls_back_to_local() {
        let local_calls = Rc::new(Cell::new(0));
        let unsupported_remote_calls = Rc::new(Cell::new(0));
        let mut dispatcher = ComputeDispatcher::new(
            manifest("local", 2, 0, vec![TranscriptionModel::Whisper]),
            MockTranscriptionEngine::new("local", Rc::clone(&local_calls)),
        );
        dispatcher.register_remote_engine(
            manifest("remote", 32, 48_000, vec![]),
            MockTranscriptionEngine::new("remote", Rc::clone(&unsupported_remote_calls)),
        );

        let transcript = dispatcher
            .dispatch_transcription(
                &test_recording(),
                TranscriptionModel::Whisper,
                &ComputePreference::Automatic,
            )
            .unwrap_or_else(|error| panic!("local fallback should succeed: {error}"));

        assert_eq!(transcript.content(), "mock transcript from local");
        assert_eq!(local_calls.get(), 1);
        assert_eq!(unsupported_remote_calls.get(), 0);
    }

    #[test]
    fn unavailable_specific_peer_returns_a_routing_error_without_invoking_an_engine() {
        let local_calls = Rc::new(Cell::new(0));
        let dispatcher = ComputeDispatcher::new(
            manifest("local", 2, 0, vec![TranscriptionModel::Whisper]),
            MockTranscriptionEngine::new("local", Rc::clone(&local_calls)),
        );

        let dispatch_result = dispatcher.dispatch_transcription(
            &test_recording(),
            TranscriptionModel::Whisper,
            &ComputePreference::SpecificPeer(DeviceId::new("missing-peer")),
        );

        assert!(matches!(
            dispatch_result,
            Err(ComputeDispatchError::PeerUnavailable { .. })
        ));
        assert_eq!(local_calls.get(), 0);
    }
}
