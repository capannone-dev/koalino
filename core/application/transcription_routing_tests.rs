//! Specifies routing: the owner's choice decides where a chunk goes, and any remote failure falls
//! back to the local engine with the audio intact, while a remote success deletes it.

use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex, PoisonError},
};

use super::{
    compute_router::ComputePreference,
    transcription_routing::{
        RemoteTranscriptionGateway, RoutedTranscriptionEngine, parse_route, route_text,
    },
};
use crate::{
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, DeviceManifest, MeetingId, RecordingId, Transcript,
            TranscriptionContext, TranscriptionModel,
        },
    },
    ports::{ai::TranscriptionEngine, remote_transcription::ChunkSamples},
};

/// A fake P2P task.
struct FakeGateway {
    route: ComputePreference,
    peers: Vec<DeviceManifest>,
    local_models: Vec<TranscriptionModel>,
    answer: Result<String, ()>,
    calls: Mutex<Vec<(DeviceId, Option<String>)>>,
}

impl FakeGateway {
    /// A gateway with one strong device that answers `remote words`.
    fn new(route: ComputePreference) -> Arc<Self> {
        Arc::new(Self {
            route,
            peers: vec![manifest("strong", 32, vec![TranscriptionModel::Whisper])],
            local_models: vec![TranscriptionModel::Whisper],
            answer: Ok("remote words".to_owned()),
            calls: Mutex::new(Vec::new()),
        })
    }

    /// The devices asked so far.
    fn asked(&self) -> Vec<DeviceId> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|call| call.0.clone())
            .collect()
    }
}

impl RemoteTranscriptionGateway for FakeGateway {
    fn route(&self) -> ComputePreference {
        self.route.clone()
    }
    fn local_manifest(&self) -> DeviceManifest {
        manifest("this-device", 2, self.local_models.clone())
    }
    fn connected_manifests(&self) -> Vec<DeviceManifest> {
        self.peers.clone()
    }
    fn transcribe_on_peer(
        &self,
        peer: &DeviceId,
        _samples: Vec<i16>,
        previous_text: Option<String>,
    ) -> Result<String, TranscriptionError> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((peer.clone(), previous_text));
        self.answer
            .clone()
            .map_err(|()| TranscriptionError::Unavailable)
    }
}

/// A manifest with `cores` CPU cores.
fn manifest(name: &str, cores: u16, models: Vec<TranscriptionModel>) -> DeviceManifest {
    DeviceManifest::new(DeviceId::new(name), cores, 0, models)
}

/// A store holding one chunk.
#[derive(Default)]
struct FakeSamples(Mutex<HashMap<String, Vec<i16>>>);

impl ChunkSamples for FakeSamples {
    fn copy_samples(&self, recording_id: &RecordingId) -> Option<Vec<i16>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(recording_id.as_str())
            .cloned()
    }
    fn delete_samples(&self, recording_id: &RecordingId) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(recording_id.as_str());
    }
}

/// A local engine that counts its runs.
struct FakeLocal(Rc<Cell<u32>>);

impl TranscriptionEngine for FakeLocal {
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.0.set(self.0.get() + 1);
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            "local words",
        ))
    }
}

/// The chunk under test.
fn recording() -> AudioRecording {
    AudioRecording::new(
        RecordingId::new("chunk"),
        MeetingId::new("meeting"),
        DeviceId::new("this-device"),
    )
}

/// A samples store holding the chunk.
fn samples() -> Arc<FakeSamples> {
    let store = FakeSamples::default();
    store
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert("chunk".to_owned(), vec![1, 2]);
    Arc::new(store)
}

/// Transcribes the chunk through a routed engine with a local engine.
fn run(gateway: Arc<FakeGateway>, store: Arc<FakeSamples>) -> (String, u32) {
    let local_runs = Rc::new(Cell::new(0));
    let engine =
        RoutedTranscriptionEngine::routed(Some(FakeLocal(Rc::clone(&local_runs))), gateway, store);
    let transcript = engine
        .transcribe(
            &recording(),
            &TranscriptionContext::new(Some("before".to_owned())),
        )
        .unwrap_or_else(|error| panic!("the chunk must be transcribed: {error}"));
    (transcript.content().to_owned(), local_runs.get())
}

#[test]
fn local_never_touches_the_network() {
    let gateway = FakeGateway::new(ComputePreference::LocalOnly);

    let (text, local_runs) = run(Arc::clone(&gateway), samples());

    assert_eq!((text.as_str(), local_runs), ("local words", 1));
    assert!(gateway.asked().is_empty());
}

#[test]
fn a_chosen_device_transcribes_with_context_and_the_audio_is_deleted() {
    let gateway = FakeGateway::new(ComputePreference::SpecificPeer(DeviceId::new("strong")));
    let store = samples();

    let (text, local_runs) = run(Arc::clone(&gateway), Arc::clone(&store));

    assert_eq!((text.as_str(), local_runs), ("remote words", 0));
    assert_eq!(
        gateway.calls.lock().unwrap_or_else(PoisonError::into_inner)[0],
        (DeviceId::new("strong"), Some("before".to_owned()))
    );
    assert_eq!(store.copy_samples(&RecordingId::new("chunk")), None);
}

#[test]
fn auto_picks_the_strongest_device() {
    let gateway = FakeGateway::new(ComputePreference::Automatic);

    let (text, _) = run(Arc::clone(&gateway), samples());

    assert_eq!(text, "remote words");
    assert_eq!(gateway.asked(), vec![DeviceId::new("strong")]);
}

#[test]
fn a_remote_failure_falls_back_to_local_with_the_audio_intact() {
    let gateway = Arc::new(FakeGateway {
        answer: Err(()),
        ..Arc::into_inner(FakeGateway::new(ComputePreference::Automatic))
            .unwrap_or_else(|| panic!("the gateway has one owner"))
    });
    let store = samples();

    let (text, local_runs) = run(Arc::clone(&gateway), Arc::clone(&store));

    assert_eq!((text.as_str(), local_runs), ("local words", 1));
    assert_eq!(gateway.asked().len(), 1);
    assert!(store.copy_samples(&RecordingId::new("chunk")).is_some());
}

#[test]
fn a_chosen_device_that_is_not_connected_falls_back_to_local() {
    let gateway = FakeGateway::new(ComputePreference::SpecificPeer(DeviceId::new("away")));

    let (text, local_runs) = run(Arc::clone(&gateway), samples());

    assert_eq!((text.as_str(), local_runs), ("local words", 1));
    assert!(gateway.asked().is_empty());
}

#[test]
fn a_device_without_a_local_engine_uses_others_and_reports_when_it_cannot() {
    let connected = Arc::new(FakeGateway {
        local_models: Vec::new(),
        ..Arc::into_inner(FakeGateway::new(ComputePreference::Automatic))
            .unwrap_or_else(|| panic!("the gateway has one owner"))
    });
    let engine = RoutedTranscriptionEngine::<FakeLocal>::routed(None, connected, samples());
    assert_eq!(
        engine
            .transcribe(&recording(), &TranscriptionContext::default())
            .map(|transcript| transcript.content().to_owned())
            .ok(),
        Some("remote words".to_owned())
    );

    let alone = Arc::new(FakeGateway {
        peers: Vec::new(),
        ..Arc::into_inner(FakeGateway::new(ComputePreference::Automatic))
            .unwrap_or_else(|| panic!("the gateway has one owner"))
    });
    let engine = RoutedTranscriptionEngine::<FakeLocal>::routed(None, alone, samples());
    assert!(matches!(
        engine.transcribe(&recording(), &TranscriptionContext::default()),
        Err(TranscriptionError::Unavailable)
    ));
}

#[test]
fn routes_read_back_from_their_text() {
    for route in [
        ComputePreference::LocalOnly,
        ComputePreference::Automatic,
        ComputePreference::SpecificPeer(DeviceId::new("12D3KooW")),
    ] {
        assert_eq!(parse_route(&route_text(&route)), route);
    }
}
