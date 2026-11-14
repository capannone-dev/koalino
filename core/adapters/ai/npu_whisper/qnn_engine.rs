//! The NPU Whisper engine (#75): Qualcomm's precompiled `whisper_small_quantized` on ONNX
//! Runtime's QNN provider, with no CPU fallback.
//!
//! The model folder holds `encoder.onnx`/`.bin`, `decoder.onnx`/`.bin` and `metadata.json` (the
//! encoder input's and the logits' quantization). The QNN folder holds `libQnnHtp.so` and the HTP
//! skeleton the NPU loads (found through `ADSP_LIBRARY_PATH`, which the Android host sets at
//! start-up). The decoder keeps a 200-token window: its self-attention caches start at the
//! quantized zero (128), and the attention mask, right-aligned, opens one position per step.
//! `transcriber` runs the windows, with timestamps, the chunk's prompt and approximate word times.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use ort::{
    ep::{self, qnn::PerformanceMode},
    session::Session,
    value::{DynValue, Tensor, TensorElementType, ValueType},
};

use super::{
    ggml_vocab::WhisperVocab,
    greedy_decoder::{LogitScale, NpuError, WhisperNpu},
    transcriber::{WindowSettings, transcribe_windows},
};
use crate::{
    adapters::ai::local_whisper_cpp::RecordingPcmSource,
    domain::models::{Transcript, TranscriptionContext},
    domain::{errors::TranscriptionError, models::AudioRecording},
    ports::ai::TranscriptionEngine,
};

/// Where the engine's files are.
#[derive(Clone, Debug)]
pub struct NpuWhisperFiles {
    /// The precompiled model's folder.
    pub model_dir: PathBuf,
    /// The folder with `libQnnHtp.so` and its companions.
    pub qnn_dir: PathBuf,
    /// A multilingual whisper.cpp model, read for its mel filters and vocabulary only.
    pub vocabulary_model: PathBuf,
}

/// The encoder and decoder sessions and the decoder's inputs, kept between steps.
pub struct QnnWhisperNpu {
    encoder: Session,
    decoder: Session,
    encoder_input: (String, Vec<usize>),
    decoder_inputs: Vec<(String, DynValue)>,
    decoder_index: HashMap<String, usize>,
    mask_length: usize,
}

impl QnnWhisperNpu {
    /// Opens both sessions on the NPU.
    ///
    /// Inputs: The engine's files. Outputs: The NPU, or why it could not open (a missing file, a
    /// phone without a supported NPU, a QNN library that will not load).
    /// Side effects: Loads ONNX Runtime and the QNN libraries, and the model onto the NPU (about
    /// 2 s).
    pub fn open(files: &NpuWhisperFiles) -> Result<Self, NpuError> {
        #[cfg(any(windows, target_os = "android"))]
        {
            let library = if cfg!(windows) {
                "onnxruntime.dll"
            } else {
                "libonnxruntime.so"
            };
            let _environment = ort::init_from(library).map_err(failed)?;
        }
        let backend = files.qnn_dir.join("libQnnHtp.so");
        let encoder = open_session(&files.model_dir.join("encoder.onnx"), &backend)?;
        let decoder = open_session(&files.model_dir.join("decoder.onnx"), &backend)?;
        let encoder_input = encoder
            .inputs()
            .first()
            .and_then(|input| Some((input.name().to_owned(), tensor_shape(input.dtype())?.1)))
            .ok_or_else(|| NpuError(String::from("the encoder has no tensor input")))?;
        let mut decoder_inputs = Vec::new();
        for input in decoder.inputs() {
            let (element, shape) = tensor_shape(input.dtype())
                .ok_or_else(|| NpuError(format!("{} is not a tensor", input.name())))?;
            decoder_inputs.push((input.name().to_owned(), zeros(element, &shape)?));
        }
        let decoder_index: HashMap<String, usize> = decoder_inputs
            .iter()
            .enumerate()
            .map(|(index, (name, _))| (name.clone(), index))
            .collect();
        let mask_length = decoder
            .inputs()
            .iter()
            .find(|input| input.name() == "attention_mask")
            .and_then(|input| tensor_shape(input.dtype()))
            .and_then(|(_, shape)| shape.last().copied())
            .ok_or_else(|| NpuError(String::from("the decoder has no attention mask")))?;
        Ok(Self {
            encoder,
            decoder,
            encoder_input,
            decoder_inputs,
            decoder_index,
            mask_length,
        })
    }

    /// The most positions the decoder's window holds.
    pub fn max_positions(&self) -> usize {
        self.mask_length.saturating_sub(1)
    }

    /// A decoder input's data, for writing.
    fn input_mut<T: ort::value::PrimitiveTensorElementType>(
        &mut self,
        name: &str,
    ) -> Result<&mut [T], NpuError> {
        let index = *self
            .decoder_index
            .get(name)
            .ok_or_else(|| NpuError(format!("the decoder has no input {name}")))?;
        let (_, data) = self.decoder_inputs[index]
            .1
            .try_extract_tensor_mut::<T>()
            .map_err(failed)?;
        Ok(data)
    }
}

impl WhisperNpu for QnnWhisperNpu {
    fn encode(&mut self, mel: &[u16]) -> Result<(), NpuError> {
        let (name, shape) = self.encoder_input.clone();
        let input = Tensor::from_array((shape, mel.to_vec())).map_err(failed)?;
        let outputs = self
            .encoder
            .run(vec![(name.as_str(), DynValue::from(input))])
            .map_err(failed)?;
        for (output_name, value) in outputs.iter() {
            let (_, data) = value.try_extract_tensor::<u8>().map_err(failed)?;
            let index = *self
                .decoder_index
                .get(output_name)
                .ok_or_else(|| NpuError(format!("the decoder has no input {output_name}")))?;
            let (_, into) = self.decoder_inputs[index]
                .1
                .try_extract_tensor_mut::<u8>()
                .map_err(failed)?;
            let count = into.len().min(data.len());
            into[..count].copy_from_slice(&data[..count]);
        }
        drop(outputs);
        self.restart()
    }

    fn restart(&mut self) -> Result<(), NpuError> {
        // Empty self-attention caches hold the quantized zero; the whole window starts masked.
        for (name, value) in &mut self.decoder_inputs {
            if name.contains("_cache_self_") {
                let (_, data) = value.try_extract_tensor_mut::<u8>().map_err(failed)?;
                data.fill(128);
            }
        }
        self.input_mut::<u16>("attention_mask")?.fill(0);
        Ok(())
    }

    fn step(&mut self, token: u32, position: usize) -> Result<Vec<u16>, NpuError> {
        let mask_length = self.mask_length;
        if position + 1 >= mask_length {
            return Err(NpuError(String::from("the decoder's window is full")));
        }
        self.input_mut::<i32>("input_ids")?[0] = i32::try_from(token).map_err(failed)?;
        self.input_mut::<i32>("position_ids")?[0] = i32::try_from(position).map_err(failed)?;
        // Quantized 0 (attend) at this step's place, counted from the right.
        self.input_mut::<u16>("attention_mask")?[mask_length - 1 - position] = u16::MAX;
        let inputs: Vec<(&str, &DynValue)> = self
            .decoder_inputs
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect();
        let outputs = self.decoder.run(inputs).map_err(failed)?;
        let mut logits = Vec::new();
        for (name, value) in outputs.iter() {
            if name == "logits" {
                logits = value
                    .try_extract_tensor::<u16>()
                    .map_err(failed)?
                    .1
                    .to_vec();
            } else if let Some(prefix) = name.strip_suffix("_out") {
                let (_, data) = value.try_extract_tensor::<u8>().map_err(failed)?;
                let input = format!("{prefix}_in");
                if let Some(index) = self.decoder_index.get(&input) {
                    let (_, into) = self.decoder_inputs[*index]
                        .1
                        .try_extract_tensor_mut::<u8>()
                        .map_err(failed)?;
                    let count = into.len().min(data.len());
                    into[..count].copy_from_slice(&data[..count]);
                }
            }
        }
        if logits.is_empty() {
            return Err(NpuError(String::from("the decoder gave no logits")));
        }
        Ok(logits)
    }
}

/// Opens one session on the NPU only.
fn open_session(model: &Path, backend: &Path) -> Result<Session, NpuError> {
    if !model.is_file() || !backend.is_file() {
        return Err(NpuError(String::from("the NPU files are missing")));
    }
    let provider = ep::QNN::default()
        .with_backend_path(backend.to_string_lossy())
        .with_performance_mode(PerformanceMode::Burst)
        .build()
        .error_on_failure();
    Session::builder()
        .map_err(failed)?
        .with_execution_providers([provider])
        .map_err(failed)?
        .with_config_entry("session.disable_cpu_ep_fallback", "1")
        .map_err(failed)?
        .commit_from_file(model)
        .map_err(failed)
}

/// A tensor's element type and fixed shape.
fn tensor_shape(dtype: &ValueType) -> Option<(TensorElementType, Vec<usize>)> {
    match dtype {
        ValueType::Tensor { ty, shape, .. } => Some((
            *ty,
            shape
                .iter()
                .map(|dimension| usize::try_from(*dimension).ok())
                .collect::<Option<Vec<_>>>()?,
        )),
        _ => None,
    }
}

/// A zero-filled tensor of a supported element type.
fn zeros(element: TensorElementType, shape: &[usize]) -> Result<DynValue, NpuError> {
    let count = shape.iter().product::<usize>();
    let shape = shape.to_vec();
    Ok(match element {
        TensorElementType::Uint8 => Tensor::from_array((shape, vec![0_u8; count]))
            .map_err(failed)?
            .into_dyn(),
        TensorElementType::Uint16 => Tensor::from_array((shape, vec![0_u16; count]))
            .map_err(failed)?
            .into_dyn(),
        TensorElementType::Int32 => Tensor::from_array((shape, vec![0_i32; count]))
            .map_err(failed)?
            .into_dyn(),
        other => return Err(NpuError(format!("unexpected input type {other:?}"))),
    })
}

/// Wraps an error as an NPU failure.
fn failed(error: impl std::fmt::Display) -> NpuError {
    NpuError(error.to_string())
}

/// Reads the model's `metadata.json`.
fn model_metadata(model_dir: &Path) -> Result<serde_json::Value, NpuError> {
    let text = fs::read_to_string(model_dir.join("metadata.json")).map_err(failed)?;
    serde_json::from_str(&text).map_err(failed)
}

/// A tensor's quantization (step and zero point) from its metadata entry.
fn quantization(tensor: &serde_json::Value) -> Result<(f32, i32), NpuError> {
    let parameters = &tensor["quantization_parameters"];
    match (
        parameters["scale"].as_f64(),
        parameters["zero_point"].as_i64(),
    ) {
        (Some(scale), Some(zero_point)) => {
            Ok((scale as f32, i32::try_from(zero_point).map_err(failed)?))
        }
        _ => Err(NpuError(String::from(
            "the model's metadata has no quantization for a tensor",
        ))),
    }
}

/// Transcribes recordings on the NPU.
pub struct QnnWhisperEngine {
    npu: Mutex<QnnWhisperNpu>,
    vocabulary: WhisperVocab,
    scale: f32,
    zero_point: i32,
    logit_scale: LogitScale,
    language: Option<String>,
    source: std::sync::Arc<dyn RecordingPcmSource>,
}

impl QnnWhisperEngine {
    /// Opens the engine.
    ///
    /// Inputs: The files, the default language (`None`: detected per window) and the audio
    /// source. Outputs: The engine, or why the NPU could not be used. Side effects: Reads the
    /// vocabulary model's header and loads the model onto the NPU.
    pub fn open(
        files: &NpuWhisperFiles,
        language: Option<String>,
        source: std::sync::Arc<dyn RecordingPcmSource>,
    ) -> Result<Self, NpuError> {
        let metadata = model_metadata(&files.model_dir)?;
        let (scale, zero_point) =
            quantization(&metadata["model_files"]["encoder.onnx"]["inputs"]["input_features"])?;
        let (logit_step, logit_zero) =
            quantization(&metadata["model_files"]["decoder.onnx"]["outputs"]["logits"])?;
        let model = fs::File::open(&files.vocabulary_model).map_err(failed)?;
        let vocabulary = WhisperVocab::read(&mut std::io::BufReader::new(model)).map_err(failed)?;
        if vocabulary.filter_shape() != (super::log_mel::N_MELS, 201)
            || !vocabulary.is_multilingual()
        {
            return Err(NpuError(String::from(
                "the vocabulary model is not a multilingual Whisper model with 80 mel bands",
            )));
        }
        Ok(Self {
            npu: Mutex::new(QnnWhisperNpu::open(files)?),
            vocabulary,
            scale,
            zero_point,
            logit_scale: LogitScale {
                scale: logit_step,
                zero_point: logit_zero,
            },
            language: language.filter(|code| code != "auto"),
            source,
        })
    }
}

impl TranscriptionEngine for QnnWhisperEngine {
    /// Inputs: The recording and its context (the language). Outputs: The text, without word
    /// timing. Side effects: Reads (and consumes) the recording's audio and runs the NPU.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        let samples = self.source.load_pcm_16khz_mono(audio_recording)?;
        if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
            return Err(TranscriptionError::InvalidRecording {
                reason: String::from("the audio source returned no usable samples"),
            });
        }
        let language = transcription_context.language_or(self.language.as_deref());
        // The chunk's previous text and the glossary, so names come out as the owner wrote them.
        let prompt = transcription_context
            .prompt()
            // With a leading space, as Whisper encodes a prompt.
            .map(|text| self.vocabulary.tokenize(&format!(" {}", text.trim())))
            .unwrap_or_default();
        let mut npu = self
            .npu
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?;
        let settings = WindowSettings {
            language,
            prompt,
            input_scale: self.scale,
            input_zero_point: self.zero_point,
            logit_scale: self.logit_scale,
            max_positions: npu.max_positions(),
        };
        let transcript = transcribe_windows(&mut *npu, &self.vocabulary, &samples, &settings)
            .map_err(|error| {
                tracing::warn!(%error, "NPU transcription failed");
                TranscriptionError::Unavailable
            })?;
        Ok(
            Transcript::new(audio_recording.meeting_id().clone(), transcript.text)
                .with_timed_words(transcript.words),
        )
    }
}
