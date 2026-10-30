//! Specifies the ONNX Runtime models: a missing model file is reported as unavailable, and (with
//! exported weights, `#[ignore]`d) the real models produce the shapes the engine expects.
//!
//! Run the real-weights test after `tools/diarization/export_models.py` with
//! `NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL` and `NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL` set:
//! `cargo test --features diarization -- --ignored onnx_models`.

use std::path::PathBuf;

use super::{
    onnx_models::{OnnxSegmentationModel, OnnxSpeakerEmbeddingModel},
    powerset::active_speakers,
    windowed_engine::{
        DiarizationTuning, SegmentationModel, SpeakerEmbeddingModel, WINDOW_SAMPLES,
        WindowedDiarizationEngine,
    },
};
use crate::domain::errors::DiarizationError;

/// A model path that cannot exist.
fn missing_model() -> PathBuf {
    std::env::temp_dir().join(format!("note-taker-missing-{}.onnx", uuid::Uuid::new_v4()))
}

/// Reads a model path from the environment, failing the ignored test when it is unset.
fn model_from_environment(variable: &str) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("set {variable} to an exported model"))
}

#[test]
fn a_missing_model_file_is_unavailable() {
    assert!(matches!(
        OnnxSegmentationModel::load(&missing_model()),
        Err(DiarizationError::Unavailable)
    ));
    assert!(matches!(
        OnnxSpeakerEmbeddingModel::load(&missing_model()),
        Err(DiarizationError::Unavailable)
    ));
}

#[test]
#[ignore = "needs exported models; see the module documentation"]
fn real_models_produce_the_expected_shapes() {
    let segmentation = OnnxSegmentationModel::load(&model_from_environment(
        "NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL",
    ))
    .unwrap_or_else(|error| panic!("segmentation must load: {error}"));
    let embedding = OnnxSpeakerEmbeddingModel::load(&model_from_environment(
        "NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL",
    ))
    .unwrap_or_else(|error| panic!("embedding must load: {error}"));

    let silence = vec![0.0; WINDOW_SAMPLES];
    let frames = segmentation
        .segment(&silence)
        .unwrap_or_else(|error| panic!("segmentation must run: {error}"));
    assert!(frames.len() > 500, "about 589 frames per 10 s window");
    assert!(
        frames
            .iter()
            .all(|frame| active_speakers(frame) == [false; 3])
    );

    let tone: Vec<f32> = (0..3 * 16_000)
        .map(|sample| 0.2 * (sample as f32 * 2.0 * std::f32::consts::PI * 150.0 / 16_000.0).sin())
        .collect();
    let vector = embedding
        .embed(&tone)
        .unwrap_or_else(|error| panic!("embedding must run: {error}"));
    assert_eq!(vector.len(), 192);
    assert!(vector.iter().all(|value| value.is_finite()));

    let engine =
        WindowedDiarizationEngine::new(segmentation, embedding, DiarizationTuning::default());
    let turns = engine
        .diarize_samples(&vec![0.0; 15 * 16_000])
        .unwrap_or_else(|error| panic!("silence must diarize: {error}"));
    assert!(turns.is_empty());
}
