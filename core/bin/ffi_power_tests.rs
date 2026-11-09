//! Specifies how the app's charging reports reach the after-meeting pass (#87): a phone runs the
//! pass only while charging, so the runtime passes each report to the pass's control, a pipeline
//! built later starts with the last report, and charging with speech waiting from an earlier run
//! starts the pass without a new recording. Nothing is built when no speech waits.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::{FFI_SUCCESS, FfiRuntime, test_recording_controller};
use crate::{
    adapters::audio::{
        retained_audio_store::{RetainedAudioStore, RetainedSpeechIndex},
        retained_speech_writer::RetainedSpeechWriter,
    },
    application::{crypto::voice_encryptor::ephemeral_cipher, transcript_refinement::ChunkSpan},
    domain::models::MeetingId,
    recording_controller::TranscriptPass,
    refinement_worker::RefinementControl,
};

/// A fresh folder for one test's retained speech.
fn speech_folder() -> PathBuf {
    let path = std::env::temp_dir().join(format!("nt-ffi-power-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap_or_else(|error| panic!("{error}"));
    path
}

/// A phone's runtime whose pipeline carries `control`, counting how often it is built.
fn phone_runtime(
    control: &Arc<RefinementControl>,
    folder: &std::path::Path,
    builds: &Arc<AtomicUsize>,
) -> FfiRuntime {
    let control = Arc::clone(control);
    let store = Arc::new(RetainedAudioStore::new(
        folder.to_path_buf(),
        ephemeral_cipher(),
    ));
    let builds = Arc::clone(builds);
    FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
        .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"))
        .with_recording_controller_builder(Box::new(move || {
            builds.fetch_add(1, Ordering::SeqCst);
            let writer = Arc::new(RetainedSpeechWriter::spawn(Arc::clone(&store))?);
            Ok(
                test_recording_controller(None)?.with_transcript_pass(TranscriptPass {
                    writer,
                    control: Arc::clone(&control),
                }),
            )
        }))
}

#[test]
fn charging_reports_reach_a_running_pass() {
    let folder = speech_folder();
    let control = Arc::new(RefinementControl::new(true));
    let builds = Arc::new(AtomicUsize::new(0));
    let mut runtime = phone_runtime(&control, &folder, &builds);
    assert_eq!(runtime.start_recording(), FFI_SUCCESS);
    assert_eq!(runtime.stop_recording(), FFI_SUCCESS);

    runtime.report_charging(true);
    assert!(control.is_charging());
    runtime.report_charging(false);
    assert!(!control.is_charging());
    assert_eq!(builds.load(Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_pipeline_built_later_starts_with_the_last_report() {
    let folder = speech_folder();
    let control = Arc::new(RefinementControl::new(true));
    let builds = Arc::new(AtomicUsize::new(0));
    let mut runtime = phone_runtime(&control, &folder, &builds);

    runtime.report_charging(true);
    assert_eq!(
        builds.load(Ordering::SeqCst),
        0,
        "no speech waits, so nothing is built"
    );
    assert_eq!(runtime.start_recording(), FFI_SUCCESS);

    assert_eq!(builds.load(Ordering::SeqCst), 1);
    assert!(control.is_charging());
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn charging_with_speech_waiting_starts_the_pass() {
    let folder = speech_folder();
    RetainedAudioStore::new(folder.clone(), ephemeral_cipher())
        .append_chunk(
            &MeetingId::new("meeting-left-over"),
            ChunkSpan {
                start_ms: 0,
                end_ms: 1_000,
            },
            None,
            &[0; 160],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let control = Arc::new(RefinementControl::new(true));
    let builds = Arc::new(AtomicUsize::new(0));
    let mut runtime = phone_runtime(&control, &folder, &builds)
        .with_retained_speech(RetainedSpeechIndex::new(folder.clone()));

    runtime.report_charging(false);
    assert_eq!(
        builds.load(Ordering::SeqCst),
        0,
        "not charging: nothing runs"
    );
    runtime.report_charging(true);
    runtime.report_charging(true);

    assert_eq!(builds.load(Ordering::SeqCst), 1, "built once, then reused");
    assert!(control.is_charging());
    let _ = std::fs::remove_dir_all(&folder);
}
