//! Composition root for the meeting pipeline: owns every pipeline port once and opens one
//! `MeetingSession` per recording.
//!
//! Entry points (FFI, CLI, daemon) build the factory once at start-up with whatever engines the
//! device has, then call `create_session` each time a recording starts. Sessions borrow the
//! factory's ports, so engines (and loaded models) are reused across recordings. Missing optional
//! dependencies degrade instead of failing: no diarization engine yields `Unattributed` segments,
//! and no voice matcher treats every diarized voice as unknown. The factory depends only on ports.

use std::rc::Rc;

use crate::{
    domain::{
        errors::{DiarizationError, StorageError},
        models::{MeetingId, VoiceMatchResult},
        transcription_language::TranscriptionLanguage,
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment, TranscriptionEngine},
        storage::NoteRepository,
        voice::VoiceIdentityMatcher,
    },
};

use crate::application::{
    meeting_crdt::MeetingCrdtService, unknown_speaker_clusterer::ClusteringConfig,
};

use super::{
    meeting_session::{MeetingSession, MeetingSessionError},
    task_metrics::TaskMetricsRecorder,
};

/// Placeholder diarization port for factories built without a diarization engine.
///
/// It is never called: the factory passes `None` to the session instead.
pub struct NoDiarizationEngine;

#[async_trait::async_trait]
impl DiarizationEngine for NoDiarizationEngine {
    /// Reports diarization as unavailable.
    ///
    /// Inputs: Ignored audio bytes.
    /// Outputs: Always `DiarizationError::Unavailable`.
    /// Side effects: None.
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Err(DiarizationError::Unavailable)
    }

    /// Reports no voice windows, so the final pass leaves the live result alone.
    async fn voice_windows(
        &self,
        _audio_bytes: &[u8],
    ) -> Result<Vec<crate::ports::ai::SpeakerSegment>, crate::domain::errors::DiarizationError>
    {
        Ok(Vec::new())
    }
}

/// Placeholder voice matcher for factories built without a voice-signature library.
pub struct NoVoiceMatcher;

impl VoiceIdentityMatcher for NoVoiceMatcher {
    /// Reports that no enrolled voice matches.
    ///
    /// Inputs: Ignored embedding.
    /// Outputs: Always `NoMatch`.
    /// Side effects: None.
    fn match_embedding(&self, _embedding: &[f32]) -> Result<VoiceMatchResult, StorageError> {
        Ok(VoiceMatchResult::NoMatch)
    }
}

/// Wraps a voice matcher that may be missing; a missing matcher reports `NoMatch`.
pub struct OptionalVoiceMatcher<MatcherPort> {
    voice_identity_matcher: Option<MatcherPort>,
}

impl<MatcherPort> OptionalVoiceMatcher<MatcherPort> {
    /// Reports whether a real matcher is configured.
    ///
    /// Inputs: None.
    /// Outputs: `true` when matches are looked up in a voice-signature library.
    /// Side effects: None.
    pub fn is_available(&self) -> bool {
        self.voice_identity_matcher.is_some()
    }
}

impl<MatcherPort> VoiceIdentityMatcher for OptionalVoiceMatcher<MatcherPort>
where
    MatcherPort: VoiceIdentityMatcher,
{
    /// Delegates to the configured matcher, or reports `NoMatch` when none is configured.
    ///
    /// Inputs: `embedding` is derived biometric data and must never be logged.
    /// Outputs: The match classification, or the matcher's storage error.
    /// Side effects: Queries the configured matcher's voice store, if any.
    fn match_embedding(&self, embedding: &[f32]) -> Result<VoiceMatchResult, StorageError> {
        match &self.voice_identity_matcher {
            Some(voice_identity_matcher) => voice_identity_matcher.match_embedding(embedding),
            None => Ok(VoiceMatchResult::NoMatch),
        }
    }

    /// Delegates to the configured matcher, or reports nothing when none is configured.
    ///
    /// Inputs: A speaker's averaged embedding. Outputs: The identity and score, or `None`.
    /// Side effects: Queries the configured matcher's voice store, if any.
    fn best_voice_match(
        &self,
        embedding: &[f32],
    ) -> Result<Option<(uuid::Uuid, f32)>, StorageError> {
        match &self.voice_identity_matcher {
            Some(voice_identity_matcher) => voice_identity_matcher.best_voice_match(embedding),
            None => Ok(None),
        }
    }
}

/// Reports which optional pipeline dependencies a factory was composed with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PipelineAvailability {
    /// A diarization engine is configured; otherwise segments are `Unattributed`.
    pub diarization: bool,
    /// A voice matcher is configured; otherwise diarized voices are `Unknown`.
    pub voice_matching: bool,
}

/// A session borrowing the factory's ports for one recording.
pub type BorrowedMeetingSession<
    'factory,
    TranscriptionPort,
    DiarizationPort,
    MatcherPort,
    RepositoryPort,
> = MeetingSession<
    &'factory TranscriptionPort,
    &'factory DiarizationPort,
    &'factory OptionalVoiceMatcher<MatcherPort>,
    &'factory RepositoryPort,
>;

/// Owns the pipeline ports and opens a `MeetingSession` per recording.
pub struct PipelineFactory<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort> {
    transcription_engine: TranscriptionPort,
    /// `None` when the device has no diarization engine.
    diarization_engine: Option<DiarizationPort>,
    voice_identity_matcher: OptionalVoiceMatcher<MatcherPort>,
    note_repository: RepositoryPort,
    /// Similarity policy every session uses to group unknown voices.
    clustering_config: ClusteringConfig,
    /// Telemetry every session records into; `None` records none.
    task_metrics: Option<Rc<TaskMetricsRecorder>>,
    /// The engine's configured language, recorded for audio without a chosen one.
    default_language: Option<TranscriptionLanguage>,
}

impl<TranscriptionPort, RepositoryPort>
    PipelineFactory<TranscriptionPort, NoDiarizationEngine, NoVoiceMatcher, RepositoryPort>
{
    /// Creates a factory from the two required ports; optional ports start missing.
    ///
    /// Inputs: The transcription engine selected for this device and the note repository.
    /// Outputs: A factory without diarization or voice matching.
    /// Side effects: None.
    pub fn new(transcription_engine: TranscriptionPort, note_repository: RepositoryPort) -> Self {
        Self {
            transcription_engine,
            diarization_engine: None,
            voice_identity_matcher: OptionalVoiceMatcher {
                voice_identity_matcher: None,
            },
            note_repository,
            clustering_config: ClusteringConfig::default(),
            task_metrics: None,
            default_language: None,
        }
    }
}

impl<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
    PipelineFactory<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
{
    /// Sets or clears the diarization engine.
    ///
    /// Inputs: `diarization_engine`, or `None` when the device has none.
    /// Outputs: The factory using that engine for every later session.
    /// Side effects: None.
    pub fn with_diarization_engine<NewDiarizationPort>(
        self,
        diarization_engine: Option<NewDiarizationPort>,
    ) -> PipelineFactory<TranscriptionPort, NewDiarizationPort, MatcherPort, RepositoryPort> {
        PipelineFactory {
            transcription_engine: self.transcription_engine,
            diarization_engine,
            voice_identity_matcher: self.voice_identity_matcher,
            note_repository: self.note_repository,
            clustering_config: self.clustering_config,
            task_metrics: self.task_metrics,
            default_language: self.default_language,
        }
    }

    /// Sets or clears the voice-identity matcher.
    ///
    /// Inputs: `voice_identity_matcher`, or `None` when no voice library is available.
    /// Outputs: The factory using that matcher for every later session.
    /// Side effects: None.
    pub fn with_voice_matcher<NewMatcherPort>(
        self,
        voice_identity_matcher: Option<NewMatcherPort>,
    ) -> PipelineFactory<TranscriptionPort, DiarizationPort, NewMatcherPort, RepositoryPort> {
        PipelineFactory {
            transcription_engine: self.transcription_engine,
            diarization_engine: self.diarization_engine,
            voice_identity_matcher: OptionalVoiceMatcher {
                voice_identity_matcher,
            },
            note_repository: self.note_repository,
            clustering_config: self.clustering_config,
            task_metrics: self.task_metrics,
            default_language: self.default_language,
        }
    }

    /// Replaces the unknown-speaker clustering policy (default threshold 0.40).
    ///
    /// Inputs: A validated clustering configuration.
    /// Outputs: The factory applying it to every later session.
    /// Side effects: None.
    pub fn with_clustering_config(mut self, clustering_config: ClusteringConfig) -> Self {
        self.clustering_config = clustering_config;
        self
    }

    /// Records a telemetry row for every chunk any later session transcribes or diarizes
    /// (Issue #53).
    ///
    /// Inputs: The recorder; sessions share it.
    /// Outputs: The factory. Side effects: None.
    pub fn with_task_metrics(mut self, task_metrics: TaskMetricsRecorder) -> Self {
        self.task_metrics = Some(Rc::new(task_metrics));
        self
    }

    /// Sets the language the transcription engine is configured with.
    ///
    /// Inputs: `NOTE_TAKER_WHISPER_LANGUAGE` as parsed, or `None` when unknown.
    /// Outputs: The factory; every later session records it for audio without a chosen language.
    /// Side effects: None.
    pub fn with_default_language(
        mut self,
        default_language: Option<TranscriptionLanguage>,
    ) -> Self {
        self.default_language = default_language;
        self
    }

    /// Reports which optional dependencies are configured.
    ///
    /// Inputs: None.
    /// Outputs: Diarization and voice-matching availability.
    /// Side effects: None.
    pub fn availability(&self) -> PipelineAvailability {
        PipelineAvailability {
            diarization: self.diarization_engine.is_some(),
            voice_matching: self.voice_identity_matcher.is_available(),
        }
    }

    /// Borrows the shared transcription engine.
    pub fn transcription_engine(&self) -> &TranscriptionPort {
        &self.transcription_engine
    }

    /// Borrows the shared note repository.
    pub fn note_repository(&self) -> &RepositoryPort {
        &self.note_repository
    }
}

impl<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
    PipelineFactory<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
where
    TranscriptionPort: TranscriptionEngine,
    DiarizationPort: DiarizationEngine + Sync,
    MatcherPort: VoiceIdentityMatcher,
    RepositoryPort: NoteRepository,
{
    /// Opens a pipeline session for a recording, creating the meeting when it does not exist.
    ///
    /// Inputs: `meeting_id` names a new or existing meeting.
    /// Outputs: A session borrowing this factory's ports, or a storage error.
    /// Side effects: May create the meeting (with an empty CRDT document) in the repository, then
    /// loads its existing segments so new chunks append to them. Sessions share the factory's
    /// telemetry recorder, if any.
    pub fn create_session(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<
        BorrowedMeetingSession<'_, TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>,
        MeetingSessionError,
    > {
        if self
            .note_repository
            .load_meeting_metadata(meeting_id)?
            .is_none()
        {
            MeetingCrdtService::new(&self.note_repository).create_meeting(meeting_id)?;
            tracing::info!(meeting_id = %meeting_id.as_str(), "Meeting created for recording");
        }
        MeetingSession::start(
            meeting_id.clone(),
            &self.transcription_engine,
            self.diarization_engine.as_ref(),
            &self.voice_identity_matcher,
            &self.note_repository,
        )
        .map(|session| {
            let session = session
                .with_clustering_config(self.clustering_config)
                .with_default_language(self.default_language);
            match &self.task_metrics {
                Some(task_metrics) => session.with_task_metrics(Rc::clone(task_metrics)),
                None => session,
            }
        })
    }
}
