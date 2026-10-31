//! Runs the diarization models with ONNX Runtime (`diarization` feature, Issue #44).
//!
//! Both models are exported by `tools/diarization/export_models.py`:
//!
//! * segmentation: pyannote segmentation-3.0, waveform `[1, 1, 160000]` in, powerset scores
//!   `[1, frames, 7]` out;
//! * embedding: SpeechBrain ECAPA-TDNN with its filterbank inside the graph, waveform
//!   `[1, samples]` in, embedding `[1, 192]` out.
//!
//! Inputs and outputs are taken by position, so exports that name them differently still load.
//! ONNX Runtime sessions need exclusive access to run, so each sits behind a mutex; the engine
//! calls them from a blocking thread. Model paths come from configuration, never from the code.
//! ONNX Runtime's errors are logged (they name files and shapes, never audio); audio and
//! embeddings are not.

use std::{path::Path, sync::Mutex};

use ort::{session::Session, value::TensorRef};

use super::{
    powerset::POWERSET_CLASSES,
    windowed_engine::{SegmentationModel, SpeakerEmbeddingModel, WINDOW_SAMPLES},
};
use crate::domain::errors::DiarizationError;

/// pyannote segmentation-3.0 in ONNX Runtime.
pub struct OnnxSegmentationModel {
    session: Mutex<Session>,
}

impl OnnxSegmentationModel {
    /// Loads the segmentation model.
    ///
    /// Inputs: The path of `segmentation.onnx`.
    /// Outputs: The model, or `Unavailable` when the file is missing or not a loadable model.
    /// Side effects: Reads the file and initializes an ONNX Runtime session.
    pub fn load(model_path: &Path) -> Result<Self, DiarizationError> {
        Ok(Self {
            session: Mutex::new(load_session(model_path)?),
        })
    }
}

impl SegmentationModel for OnnxSegmentationModel {
    /// Scores the frames of one 10 s window.
    ///
    /// Outputs: Per-frame powerset scores, `InvalidAudio` for a window of the wrong length or an
    /// output of an unexpected shape, or `Unavailable` when inference fails.
    /// Side effects: Runs inference.
    fn segment(&self, window: &[f32]) -> Result<Vec<[f32; POWERSET_CLASSES]>, DiarizationError> {
        if window.len() != WINDOW_SAMPLES {
            return Err(invalid("a segmentation window must hold 10 s of audio"));
        }
        let input = TensorRef::from_array_view(([1_usize, 1, WINDOW_SAMPLES], window))
            .map_err(inference_failure)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| DiarizationError::Unavailable)?;
        let outputs = session
            .run(ort::inputs![input])
            .map_err(inference_failure)?;
        let (shape, scores) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(inference_failure)?;
        if shape.len() != 3 || shape[0] != 1 || shape[2] != POWERSET_CLASSES as i64 {
            return Err(invalid(
                "the segmentation model's output is not [1, frames, 7]",
            ));
        }
        Ok(scores.as_chunks::<POWERSET_CLASSES>().0.to_vec())
    }
}

/// SpeechBrain ECAPA-TDNN (with its filterbank) in ONNX Runtime.
pub struct OnnxSpeakerEmbeddingModel {
    session: Mutex<Session>,
}

impl OnnxSpeakerEmbeddingModel {
    /// Loads the embedding model.
    ///
    /// Inputs: The path of `embedding.onnx`.
    /// Outputs: The model, or `Unavailable` when the file is missing or not a loadable model.
    /// Side effects: Reads the file and initializes an ONNX Runtime session.
    pub fn load(model_path: &Path) -> Result<Self, DiarizationError> {
        Ok(Self {
            session: Mutex::new(load_session(model_path)?),
        })
    }
}

impl SpeakerEmbeddingModel for OnnxSpeakerEmbeddingModel {
    /// Embeds one speaker's audio.
    ///
    /// Outputs: The embedding (biometric; never logged), `InvalidAudio` for empty audio or an
    /// output that is not one vector, or `Unavailable` when inference fails.
    /// Side effects: Runs inference.
    fn embed(&self, samples: &[f32]) -> Result<Vec<f32>, DiarizationError> {
        if samples.is_empty() {
            return Err(invalid("there is no audio to embed"));
        }
        let input = TensorRef::from_array_view(([1_usize, samples.len()], samples))
            .map_err(inference_failure)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| DiarizationError::Unavailable)?;
        let outputs = session
            .run(ort::inputs![input])
            .map_err(inference_failure)?;
        let (shape, embedding) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(inference_failure)?;
        if shape.iter().rev().skip(1).any(|dimension| *dimension != 1) || embedding.is_empty() {
            return Err(invalid("the embedding model's output is not one vector"));
        }
        Ok(embedding.to_vec())
    }
}

/// Opens an ONNX Runtime session with one intra-op thread per core.
///
/// Outputs: The session, or `Unavailable` (with ONNX Runtime's reason logged).
/// Side effects: Reads the model file; on Windows loads `onnxruntime.dll` from beside the
/// executable first, because `ort` panics on first use if the library cannot be loaded.
fn load_session(model_path: &Path) -> Result<Session, DiarizationError> {
    #[cfg(windows)]
    let _environment = ort::init_from("onnxruntime.dll").map_err(load_failure)?;
    let thread_count = crate::adapters::ai::compute_resources::preferred_inference_threads();
    let mut builder = Session::builder()
        .map_err(load_failure)?
        .with_intra_threads(thread_count)
        .map_err(load_failure)?;
    builder.commit_from_file(model_path).map_err(load_failure)
}

/// Logs why a model failed to load and reports the engine as unavailable.
fn load_failure(error: impl std::fmt::Display) -> DiarizationError {
    tracing::warn!(%error, "unable to load a diarization model");
    DiarizationError::Unavailable
}

/// Logs why inference failed and reports the engine as unavailable.
fn inference_failure(error: impl std::fmt::Display) -> DiarizationError {
    tracing::warn!(%error, "diarization inference failed");
    DiarizationError::Unavailable
}

/// Builds an `InvalidAudio` error.
fn invalid(reason: &str) -> DiarizationError {
    DiarizationError::InvalidAudio {
        reason: reason.to_owned(),
    }
}
