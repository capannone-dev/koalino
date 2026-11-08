//! Implements local whisper.cpp transcription behind the portable `TranscriptionEngine` port.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, Transcript, TranscriptionContext},
    },
    ports::ai::TranscriptionEngine,
};

#[cfg(all(feature = "mobile-gpu", target_os = "android"))]
use super::compute_resources::efficient_gpu_label;
use super::compute_resources::{
    GpuDevice, choose_gpu, missing_cpu_instructions, model_fits_on_gpu,
};
use super::gpu_attempt_guard::GpuAttemptGuard;
use super::whisper_word_timing::{WhisperTokenTiming, words_from_tokens, words_match_text};

/// Resolves adapter-owned, normalized PCM samples for one portable recording identifier.
///
/// Implementations may read a temporary local audio store, but they must return exactly 16 kHz,
/// mono, floating-point PCM. This is deliberately an adapter-local seam: neither the application
/// nor the transcription port gains a whisper.cpp, file-system, or raw-audio dependency.
pub trait RecordingPcmSource: Send + Sync {
    /// Loads normalized local PCM for transcription.
    ///
    /// Inputs: `audio_recording` contains opaque, portable recording metadata.
    /// Outputs: 16 kHz mono floating-point PCM samples or a portable transcription error.
    /// Side effects: An implementation may read and consume temporary local audio; it must never
    /// log or persist the returned raw samples.
    fn load_pcm_16khz_mono(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Vec<f32>, TranscriptionError>;
}

/// Configures one local whisper.cpp model without exposing binding types to application callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalWhisperCppConfig {
    model_path: PathBuf,
    thread_count: i32,
    use_gpu: bool,
    language: Option<String>,
    efficient_gpu: bool,
    gpu_attempt_marker: Option<PathBuf>,
}

impl LocalWhisperCppConfig {
    /// Validates local inference configuration before a native model context is initialized.
    ///
    /// Inputs: `model_path` names a local GGML/GGUF model; `thread_count` is at least one;
    /// `use_gpu` requests a compiled-in GPU backend; `language` is an optional Whisper language.
    /// Outputs: A validated configuration or a portable error that contains no model contents.
    /// Side effects: Owns path and language metadata only; it does not read or download a model.
    pub fn new(
        model_path: PathBuf,
        thread_count: usize,
        use_gpu: bool,
        language: Option<String>,
    ) -> Result<Self, TranscriptionError> {
        let thread_count =
            i32::try_from(thread_count).map_err(|_| TranscriptionError::InvalidRecording {
                reason: "local transcription thread count exceeds whisper.cpp limits".to_owned(),
            })?;
        if thread_count < 1 {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local transcription requires at least one thread".to_owned(),
            });
        }
        if language
            .as_deref()
            .is_some_and(|configured_language| configured_language.contains('\0'))
        {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local transcription language must not contain a null byte".to_owned(),
            });
        }

        Ok(Self {
            model_path,
            thread_count,
            use_gpu,
            language,
            efficient_gpu: false,
            gpu_attempt_marker: None,
        })
    }

    /// Chooses efficient mode: a supported phone GPU, Flash Attention and pinned threads.
    ///
    /// Inputs: True to use it. A GPU it doesn't support runs on the CPU instead.
    /// Outputs: The configuration. Side effects: None.
    pub fn with_efficient_gpu(mut self, efficient_gpu: bool) -> Self {
        self.efficient_gpu = efficient_gpu;
        self
    }

    /// Sets the file that records an unfinished GPU attempt, so a driver crash isn't repeated.
    ///
    /// Inputs: The file's path, in a folder that exists. Outputs: The configuration.
    /// Side effects: None until a GPU is tried.
    pub fn with_gpu_attempt_marker(mut self, marker_file: PathBuf) -> Self {
        self.gpu_attempt_marker = Some(marker_file);
        self
    }
}

/// True while the latest engine runs on a GPU, for Settings to report.
static ENGINE_USES_GPU: AtomicBool = AtomicBool::new(false);

/// Reports whether the latest engine runs on a GPU.
///
/// Outputs: True once one has loaded on a GPU. Side effects: None.
pub fn engine_uses_gpu() -> bool {
    ENGINE_USES_GPU.load(Ordering::Relaxed)
}

/// Names the GPU that efficient mode can use on this device.
///
/// Outputs: A short name such as `Adreno 740`, or `None` where it can't run.
/// Side effects: Loads the phone's OpenCL library if it has one, and reads what the driver reports.
#[cfg(all(feature = "mobile-gpu", target_os = "android"))]
pub fn efficient_gpu() -> Option<String> {
    super::opencl_loader_shim::efficient_gpu_version().map(efficient_gpu_label)
}

/// Reports no such GPU where the phone GPU backend isn't built.
///
/// Outputs: `None`. Side effects: None.
#[cfg(not(all(feature = "mobile-gpu", target_os = "android")))]
pub fn efficient_gpu() -> Option<String> {
    None
}

/// Lists the GPUs whisper.cpp can use, in the order its `gpu_device` setting counts them.
///
/// Outputs: The GPUs; empty for a build without a GPU backend, or a machine without a usable GPU.
/// Side effects: Starts whisper.cpp's backends on first use, which may load the Vulkan library.
pub fn available_gpus() -> Vec<GpuDevice> {
    use whisper_rs_sys::{
        ggml_backend_dev_count, ggml_backend_dev_get, ggml_backend_dev_memory,
        ggml_backend_dev_type, ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU,
        ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_IGPU,
    };
    let mut gpus = Vec::new();
    // SAFETY: These read ggml's device list, which ggml owns and keeps for the process's lifetime.
    // The memory pointers are valid for the call.
    unsafe {
        for position in 0..ggml_backend_dev_count() {
            let device = ggml_backend_dev_get(position);
            let device_type = ggml_backend_dev_type(device);
            let is_discrete = device_type == ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU;
            if !is_discrete && device_type != ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_IGPU {
                continue;
            }
            let (mut free_memory_bytes, mut total_memory_bytes) = (0_usize, 0_usize);
            ggml_backend_dev_memory(device, &mut free_memory_bytes, &mut total_memory_bytes);
            gpus.push(GpuDevice {
                index: i32::try_from(gpus.len()).unwrap_or(i32::MAX),
                is_discrete,
                total_memory_bytes: total_memory_bytes as u64,
                free_memory_bytes: free_memory_bytes as u64,
            });
        }
    }
    gpus
}

/// Picks the GPU for a model, when one should be used.
///
/// Inputs: The configuration; a request for the CPU returns `None` at once.
/// Outputs: The GPU to try, or `None` when there is none or the model looks too large for it.
/// Side effects: See `available_gpus`; reads the model file's size.
fn gpu_for_model(configuration: &LocalWhisperCppConfig) -> Option<GpuDevice> {
    if !configuration.use_gpu {
        return None;
    }
    if configuration.efficient_gpu {
        // OpenCL reports no memory figures, and a phone's GPU shares system memory anyway.
        // OpenCL is the only GPU backend in this build, so it is the first GPU.
        return efficient_gpu().map(|_| GpuDevice {
            index: 0,
            is_discrete: false,
            total_memory_bytes: 0,
            free_memory_bytes: 0,
        });
    }
    let chosen_gpu = choose_gpu(&available_gpus())?;
    let model_bytes = std::fs::metadata(&configuration.model_path).ok()?.len();
    if model_fits_on_gpu(model_bytes, chosen_gpu.free_memory_bytes) {
        Some(chosen_gpu)
    } else {
        tracing::info!("the Whisper model is too large for this GPU, using the CPU");
        None
    }
}

/// Loads a model on the chosen GPU and proves it can allocate its working memory there.
///
/// Inputs: The model path, the GPU, and whether to use Flash Attention.
/// Outputs: The context, or `None` when the GPU can't run it.
/// Side effects: Reads the model and allocates GPU memory.
fn load_on_gpu(model_path: &str, gpu: GpuDevice, flash_attention: bool) -> Option<WhisperContext> {
    let context_parameters = WhisperContextParameters {
        use_gpu: true,
        gpu_device: gpu.index,
        flash_attn: flash_attention,
        ..WhisperContextParameters::default()
    };
    let model_context = WhisperContext::new_with_params(model_path, context_parameters).ok()?;
    model_context.create_state().ok()?;
    Some(model_context)
}

/// Implements local transcription with an adapter-owned whisper.cpp model context.
///
/// A mutex serializes native state creation and inference for this one model context. Callers that
/// require non-blocking UI or runtime behavior must invoke this synchronous port on a background
/// worker, as required by the application architecture.
pub struct LocalWhisperCppTranscriptionEngine {
    model_context: Mutex<WhisperContext>,
    thread_count: i32,
    language: Option<String>,
    recording_pcm_source: Arc<dyn RecordingPcmSource>,
    /// An unfinished GPU attempt, ended by the first chunk that succeeds.
    gpu_attempt: Mutex<Option<GpuAttemptGuard>>,
    /// The cores for the threads feeding the GPU; empty when they stay free.
    #[cfg_attr(windows, allow(dead_code))]
    pinned_cores: Vec<usize>,
}

impl LocalWhisperCppTranscriptionEngine {
    /// Initializes a local whisper.cpp model and retains the adapter-local PCM source.
    ///
    /// Inputs: `configuration` is validated local model metadata; `recording_pcm_source` resolves
    /// normalized local audio for the portable recording ID.
    /// Outputs: A transcription engine or `Unavailable` when the local native model cannot load.
    /// Side effects: Opens and initializes the configured local model; it performs no inference,
    /// network request, model download, or raw-audio logging.
    pub fn new(
        configuration: LocalWhisperCppConfig,
        recording_pcm_source: Arc<dyn RecordingPcmSource>,
    ) -> Result<Self, TranscriptionError> {
        let model_path = configuration
            .model_path
            .to_str()
            .ok_or(TranscriptionError::Unavailable)?;
        let missing_instructions = missing_cpu_instructions();
        if !missing_instructions.is_empty() {
            tracing::warn!(
                ?missing_instructions,
                "this CPU cannot run local transcription"
            );
            return Err(TranscriptionError::Unavailable);
        }
        let gpu = gpu_for_model(&configuration);
        // Without a marker there's nothing to check. With one, a crashed attempt keeps the CPU.
        let gpu_attempt = gpu.and_then(|_| {
            configuration
                .gpu_attempt_marker
                .as_deref()
                .map(GpuAttemptGuard::begin)
        });
        let gpu_allowed = !matches!(gpu_attempt, Some(None));
        if !gpu_allowed {
            tracing::warn!("a GPU attempt didn't finish last time, so this one uses the CPU");
        }
        let gpu_context = gpu
            .filter(|_| gpu_allowed)
            .and_then(|gpu| load_on_gpu(model_path, gpu, configuration.efficient_gpu));
        let uses_gpu = gpu_context.is_some();
        ENGINE_USES_GPU.store(uses_gpu, Ordering::Relaxed);
        let model_context = match gpu_context {
            Some(model_context) => {
                tracing::info!("local transcription uses the GPU");
                model_context
            }
            None => {
                tracing::info!("local transcription uses the CPU");
                let cpu_parameters = WhisperContextParameters {
                    use_gpu: false,
                    ..WhisperContextParameters::default()
                };
                WhisperContext::new_with_params(model_path, cpu_parameters)
                    .map_err(|_| TranscriptionError::Unavailable)?
            }
        };

        Ok(Self {
            model_context: Mutex::new(model_context),
            thread_count: configuration.thread_count,
            language: configuration.language,
            recording_pcm_source,
            gpu_attempt: Mutex::new(gpu_attempt.flatten().filter(|_| uses_gpu)),
            pinned_cores: if uses_gpu && configuration.efficient_gpu {
                super::compute_resources::cores_for_gpu_submission(
                    &super::compute_resources::core_max_speeds(),
                )
            } else {
                Vec::new()
            },
        })
    }

    /// Validates that adapter-provided PCM is safe for the native whisper.cpp API.
    ///
    /// Inputs: `pcm_samples` must be normalized 16 kHz mono floating-point samples.
    /// Outputs: Success or a portable invalid-recording error.
    /// Side effects: None; it does not retain or log the raw samples.
    fn validate_pcm(pcm_samples: &[f32]) -> Result<(), TranscriptionError> {
        if pcm_samples.is_empty() {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local audio source returned no PCM samples".to_owned(),
            });
        }
        if pcm_samples.iter().any(|sample| !sample.is_finite()) {
            return Err(TranscriptionError::InvalidRecording {
                reason: "local audio source returned non-finite PCM samples".to_owned(),
            });
        }
        Ok(())
    }
}

impl TranscriptionEngine for LocalWhisperCppTranscriptionEngine {
    /// Transcribes one recording through adapter-owned whisper.cpp inference.
    ///
    /// Inputs: `audio_recording` provides meeting metadata only; `transcription_context` provides
    /// optional prior text; the injected local source resolves 16 kHz mono PCM without exposing
    /// raw audio through the port.
    /// Outputs: A meeting-associated portable transcript, or an unavailable/invalid-recording
    /// error. Native whisper.cpp errors never cross the hexagonal boundary.
    /// Side effects: Reads temporary local audio through the source and runs CPU/GPU inference;
    /// it serializes this engine's model use and neither logs nor persists raw audio.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        let pcm_samples = self
            .recording_pcm_source
            .load_pcm_16khz_mono(audio_recording)?;
        Self::validate_pcm(&pcm_samples)?;

        let model_context = self
            .model_context
            .lock()
            .map_err(|_| TranscriptionError::Unavailable)?;
        let mut inference_state = model_context
            .create_state()
            .map_err(|_| TranscriptionError::Unavailable)?;
        let mut inference_parameters = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        inference_parameters.set_n_threads(self.thread_count);
        inference_parameters.set_print_progress(false);
        inference_parameters.set_print_realtime(false);
        inference_parameters.set_print_timestamps(false);
        inference_parameters.set_token_timestamps(true);
        if let Some(language) = transcription_context.language_or(self.language.as_deref()) {
            inference_parameters.set_language(Some(language));
        }
        // The glossary, so names and terms are spelled as the owner wrote them.
        let prompt = transcription_context.prompt();
        if let Some(prompt) = prompt.as_deref() {
            inference_parameters.set_initial_prompt(prompt);
        }
        {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            let _pinned_threads = super::gpu_thread_affinity::PinnedThreads::pin(
                &self.pinned_cores,
                super::gpu_thread_affinity::ADRENO_DRIVER_THREAD_PREFIX,
            );
            inference_state
                .full(inference_parameters, &pcm_samples)
                .map_err(|_| TranscriptionError::Unavailable)?;
        }
        if let Ok(mut gpu_attempt) = self.gpu_attempt.lock()
            && let Some(attempt) = gpu_attempt.take()
        {
            attempt.confirm();
        }

        let mut transcript_content = String::new();
        let mut spoken_tokens = Vec::new();
        for transcript_segment in inference_state.as_iter() {
            let segment_text = transcript_segment
                .to_str_lossy()
                .map_err(|_| TranscriptionError::Unavailable)?;
            if is_non_speech_marker(&segment_text) {
                continue;
            }
            transcript_content.push_str(&segment_text);
            for token_index in 0..transcript_segment.n_tokens() {
                if let Some(token) = transcript_segment.get_token(token_index)
                    && let Ok(bytes) = token.to_bytes()
                {
                    let timing = token.token_data();
                    spoken_tokens.push((bytes.to_vec(), timing.t0, timing.t1));
                }
            }
        }

        // Timing only steers who said a word: if it ever disagrees with the text, the text wins
        // and the transcript falls back to splitting by time.
        let timed_words =
            words_from_tokens(
                spoken_tokens
                    .iter()
                    .map(|(bytes, t0, t1)| WhisperTokenTiming {
                        bytes,
                        start_units: *t0,
                        end_units: *t1,
                    }),
            );
        let timed_words = if words_match_text(&timed_words, &transcript_content) {
            timed_words
        } else {
            tracing::warn!("whisper word timing does not match its text; ignoring the timing");
            Vec::new()
        };

        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            transcript_content.trim(),
        )
        .with_timed_words(timed_words))
    }
}

/// Recognizes a segment that is only one of whisper.cpp's non-speech markers (Issue #51).
///
/// Inputs: One segment's text.
/// Outputs: `true` for a lone bracketed, parenthesized or starred tag such as `[BLANK_AUDIO]`,
/// `(silence)` or `*applause*`, which would otherwise end up in the transcript; `false` for
/// anything with words outside such a tag, and for empty text.
/// Side effects: None.
pub fn is_non_speech_marker(segment_text: &str) -> bool {
    let trimmed = segment_text.trim();
    let mut characters = trimmed.chars();
    let (Some(opening), Some(closing)) = (characters.next(), characters.next_back()) else {
        return false;
    };
    let inner = characters.as_str();
    let expected_closing = match opening {
        '[' => ']',
        '(' => ')',
        '*' => '*',
        _ => return false,
    };
    closing == expected_closing
        && !inner.contains([opening, expected_closing])
        && !inner.trim().is_empty()
}
