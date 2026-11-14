//! Specifies the NPU Whisper engine's portable parts (#75): the log-mel input, the vocabulary and
//! mel filters read from a whisper.cpp model file, and the greedy decoder loop over a fake NPU.

use super::{
    ggml_vocab::{GgmlVocabError, WhisperVocab},
    greedy_decoder::{EOT, NpuError, SOT, WhisperNpu, decode_greedy, language_token},
    log_mel::{N_FRAMES, N_MELS, dft_power, log_mel, quantize},
};

/// A filter bank of one-hot rows: filter `m` keeps FFT bin `m`.
fn one_hot_filters() -> Vec<f32> {
    let mut filters = vec![0.0; N_MELS * 201];
    for mel in 0..N_MELS {
        filters[mel * 201 + mel] = 1.0;
    }
    filters
}

#[test]
fn the_fast_transform_matches_the_plain_one() {
    let frame: Vec<f32> = (0..400)
        .map(|n| ((n * 37 % 101) as f32 / 50.0) - 1.0)
        .collect();
    let fast = dft_power(&frame);
    for bin in [0_usize, 1, 7, 100, 200] {
        let (mut real, mut imaginary) = (0.0_f64, 0.0_f64);
        for (n, sample) in frame.iter().enumerate() {
            let angle = -2.0 * std::f64::consts::PI * (bin * n) as f64 / 400.0;
            real += f64::from(*sample) * angle.cos();
            imaginary += f64::from(*sample) * angle.sin();
        }
        let expected = real * real + imaginary * imaginary;
        assert!(
            (f64::from(fast[bin]) - expected).abs() <= 1e-3 * expected.max(1.0),
            "bin {bin}: {} vs {expected}",
            fast[bin]
        );
    }
}

#[test]
fn silence_is_flat_and_a_tone_lights_its_bin() {
    let filters = one_hot_filters();
    let silence = log_mel(&[0.0; 16_000], &filters);
    assert_eq!(silence.len(), N_MELS * N_FRAMES);
    assert!(
        silence
            .iter()
            .all(|value| (value - silence[0]).abs() < 1e-6)
    );

    // 440 Hz is FFT bin 11 (40 Hz per bin); one second of it, then 29 s of padding.
    let tone: Vec<f32> = (0..16_000)
        .map(|n| (2.0 * std::f32::consts::PI * 440.0 * n as f32 / 16_000.0).sin() * 0.5)
        .collect();
    let mel = log_mel(&tone, &filters);
    let at = |mel_bin: usize, frame: usize| mel[mel_bin * N_FRAMES + frame];
    assert!(
        at(11, 50) > at(5, 50) + 1.0,
        "the tone's bin is the loudest"
    );
    let peak = mel.iter().copied().fold(f32::MIN, f32::max);
    // Whisper floors everything at 8 (log10) below the peak, then maps x to (x + 4) / 4.
    assert!(
        (at(11, 2_000) - (peak - 2.0)).abs() < 1e-4,
        "padding sits at the floor"
    );
}

#[test]
fn quantizing_rounds_and_clamps() {
    assert_eq!(
        quantize(&[0.0, 1.0, -100.0, 100_000.0], 0.5, 10),
        vec![10, 12, 0, 65_535]
    );
}

/// A whisper.cpp model file with 2 mel filters of 3 bins and 3 tokens.
fn tiny_ggml() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(0x6767_6d6c_u32.to_le_bytes());
    for value in [3_i32, 1500, 384, 6, 4, 448, 384, 6, 4, 2, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(2_i32.to_le_bytes());
    bytes.extend(3_i32.to_le_bytes());
    for value in [0.5_f32, 0.25, 0.0, 0.0, 1.0, 2.0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(3_i32.to_le_bytes());
    for token in ["Hel", "lo", " wörld"] {
        bytes.extend((token.len() as u32).to_le_bytes());
        bytes.extend(token.as_bytes());
    }
    bytes.extend([0xAB; 64]); // the weights would follow
    bytes
}

#[test]
fn the_vocabulary_and_filters_come_from_a_whisper_cpp_model() {
    let vocab = WhisperVocab::read(&mut tiny_ggml().as_slice()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(vocab.mel_filters(), &[0.5, 0.25, 0.0, 0.0, 1.0, 2.0]);
    assert_eq!(vocab.filter_shape(), (2, 3));
    assert_eq!(vocab.text(&[0, 1, 2]), "Hello wörld");
    assert_eq!(vocab.text(&[0, 99]), "Hel", "unknown tokens are skipped");
    assert!(!vocab.is_multilingual(), "its vocabulary has 3 entries");
    let mut multilingual = tiny_ggml();
    multilingual[4..8].copy_from_slice(&51_865_i32.to_le_bytes());
    let multilingual =
        WhisperVocab::read(&mut multilingual.as_slice()).unwrap_or_else(|e| panic!("{e}"));
    assert!(multilingual.is_multilingual(), "Whisper's 51,865 tokens");

    let mut wrong = tiny_ggml();
    wrong[0] = 0;
    assert!(matches!(
        WhisperVocab::read(&mut wrong.as_slice()),
        Err(GgmlVocabError::NotAModel)
    ));
    assert!(WhisperVocab::read(&mut &tiny_ggml()[..30]).is_err());
}

/// An NPU that answers each step from a script: the token to rate highest, given the token fed.
struct ScriptedNpu {
    fed: Vec<(u32, usize)>,
    answers: Vec<u32>,
    encoded: usize,
}

impl WhisperNpu for ScriptedNpu {
    fn encode(&mut self, mel: &[u16]) -> Result<(), NpuError> {
        assert_eq!(mel.len(), N_MELS * N_FRAMES);
        self.encoded += 1;
        Ok(())
    }

    fn step(&mut self, token: u32, position: usize) -> Result<Vec<u16>, NpuError> {
        self.fed.push((token, position));
        let mut logits = vec![1_000_u16; 51_865];
        // A timestamp token always looks best; the decoder must never pick one.
        logits[50_400] = 65_535;
        if let Some(best) = self.answers.get(position) {
            logits[*best as usize] = 60_000;
        }
        // Italian a little ahead of English among the languages.
        logits[language_token("it").unwrap_or(0) as usize] =
            logits[language_token("it").unwrap_or(0) as usize].max(50_000);
        logits[language_token("en").unwrap_or(0) as usize] = 40_000;
        Ok(logits)
    }
}

#[test]
fn a_forced_language_is_decoded_greedily_until_the_end() {
    // Positions: 0 SOT, 1 language, 2 transcribe, 3 no timestamps, then text.
    let mut npu = ScriptedNpu {
        fed: Vec::new(),
        answers: vec![0, 0, 0, 7, 8, EOT],
        encoded: 0,
    };
    let decoded = decode_greedy(&mut npu, &[0; N_MELS * N_FRAMES], Some("en"), 199)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decoded.tokens, vec![7, 8]);
    assert_eq!(decoded.language, "en");
    assert_eq!(npu.encoded, 1);
    let english = language_token("en").unwrap_or(0);
    assert_eq!(
        npu.fed,
        vec![
            (SOT, 0),
            (english, 1),
            (50_359, 2),
            (50_363, 3),
            (7, 4),
            (8, 5)
        ]
    );
}

#[test]
fn without_a_language_the_model_picks_one_and_the_length_is_capped() {
    let mut npu = ScriptedNpu {
        fed: Vec::new(),
        answers: vec![0, 0, 0, 5, 5, 5, 5, 5, 5, 5],
        encoded: 0,
    };
    let decoded =
        decode_greedy(&mut npu, &[0; N_MELS * N_FRAMES], None, 8).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decoded.language, "it");
    assert_eq!(npu.fed[1], (language_token("it").unwrap_or(0), 1));
    assert_eq!(decoded.tokens.len(), 4, "8 positions, 4 of them forced");
    assert_eq!(language_token("xx"), None);
    assert_eq!(language_token("it"), Some(50_274));
}

/// Writes the log-mel of a WAV (16 kHz mono) as little-endian f32, to compare with a reference
/// implementation by hand: `NPU_MEL_WAV`, `NPU_MEL_MODEL` (a whisper.cpp model), `NPU_MEL_OUT`.
#[test]
#[ignore = "needs a WAV, a whisper.cpp model and an output path"]
fn writes_a_log_mel_for_comparison() {
    let variable = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is unset"));
    let file = std::fs::File::open(variable("NPU_MEL_WAV")).unwrap_or_else(|e| panic!("{e}"));
    let mut input = crate::adapters::audio::pcm_input::PcmInput::wav(Box::new(file))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut samples = Vec::new();
    while let Some(block) = input.read_block().unwrap_or_else(|e| panic!("{e}")) {
        samples.extend(block.iter().map(|sample| f32::from(*sample) / 32_768.0));
    }
    let model = std::fs::File::open(variable("NPU_MEL_MODEL")).unwrap_or_else(|e| panic!("{e}"));
    let vocab =
        WhisperVocab::read(&mut std::io::BufReader::new(model)).unwrap_or_else(|e| panic!("{e}"));
    let mel = log_mel(&samples, vocab.mel_filters());
    let bytes: Vec<u8> = mel.iter().flat_map(|value| value.to_le_bytes()).collect();
    std::fs::write(variable("NPU_MEL_OUT"), bytes).unwrap_or_else(|e| panic!("{e}"));
}

/// Hands the engine a WAV file's samples.
#[cfg(feature = "npu-whisper")]
struct WavSamples(Vec<f32>);

#[cfg(feature = "npu-whisper")]
impl crate::adapters::ai::local_whisper_cpp::RecordingPcmSource for WavSamples {
    fn load_pcm_16khz_mono(
        &self,
        _audio_recording: &crate::domain::models::AudioRecording,
    ) -> Result<Vec<f32>, crate::domain::errors::TranscriptionError> {
        Ok(self.0.clone())
    }
}

/// Transcribes a public 16 kHz WAV on a phone's NPU and prints the text and time taken.
///
/// Run by hand on a Snapdragon 8 Gen 2 phone, with `LD_LIBRARY_PATH` naming ONNX Runtime's folder
/// and `ADSP_LIBRARY_PATH` the QNN folder: `NPU_WAV`, `NPU_MODEL_DIR`, `NPU_QNN_DIR`,
/// `NPU_VOCAB` (a multilingual whisper.cpp model) and `NPU_LANGUAGE`.
#[cfg(feature = "npu-whisper")]
#[test]
#[ignore = "needs a phone NPU, the downloaded model and a WAV"]
fn transcribes_a_wav_on_the_npu() {
    use super::qnn_engine::{NpuWhisperFiles, QnnWhisperEngine};
    use crate::{
        domain::models::{AudioRecording, DeviceId, MeetingId, RecordingId, TranscriptionContext},
        ports::ai::TranscriptionEngine,
    };
    let variable = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is unset"));
    let file = std::fs::File::open(variable("NPU_WAV")).unwrap_or_else(|e| panic!("{e}"));
    let mut input = crate::adapters::audio::pcm_input::PcmInput::wav(Box::new(file))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut samples = Vec::new();
    while let Some(block) = input.read_block().unwrap_or_else(|e| panic!("{e}")) {
        samples.extend(block.iter().map(|sample| f32::from(*sample) / 32_768.0));
    }
    let files = NpuWhisperFiles {
        model_dir: variable("NPU_MODEL_DIR").into(),
        qnn_dir: variable("NPU_QNN_DIR").into(),
        vocabulary_model: variable("NPU_VOCAB").into(),
    };
    let opened = std::time::Instant::now();
    let engine = QnnWhisperEngine::open(
        &files,
        Some(variable("NPU_LANGUAGE")),
        std::sync::Arc::new(WavSamples(samples)),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let started = std::time::Instant::now();
    let transcript = engine
        .transcribe(
            &AudioRecording::new(
                RecordingId::new("clip"),
                MeetingId::new("meeting"),
                DeviceId::new("phone"),
            ),
            &TranscriptionContext::new(None),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    println!(
        "opened in {:?}, transcribed in {:?}: {}",
        started - opened,
        started.elapsed(),
        transcript.content()
    );
    assert!(!transcript.content().is_empty());
}

/// Creates an empty NPU folder for a test.
fn npu_folder(name: &str) -> std::path::PathBuf {
    let folder = std::env::temp_dir().join(format!("note-taker-npu-{name}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&folder));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    folder
}

#[test]
fn the_npu_counts_as_installed_only_with_every_file() {
    use super::install::{MODEL_FILES, NpuInstall, QNN_FILES};
    let folder = npu_folder("install");
    let install = NpuInstall::new(&folder);
    assert_eq!(install.model_dir(), folder.join("model"));
    assert_eq!(install.qnn_dir(), folder.join("qnn"));
    assert!(!install.is_installed(), "an empty folder");
    std::fs::create_dir_all(install.model_dir()).unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(install.qnn_dir()).unwrap_or_else(|e| panic!("{e}"));
    for file in MODEL_FILES {
        std::fs::write(install.model_dir().join(file), b"x").unwrap_or_else(|e| panic!("{e}"));
    }
    assert!(!install.is_installed(), "the QNN libraries are missing");
    for file in QNN_FILES {
        std::fs::write(install.qnn_dir().join(file), b"x").unwrap_or_else(|e| panic!("{e}"));
    }
    assert!(install.is_installed());
    std::fs::remove_file(install.model_dir().join("decoder.bin")).unwrap_or_else(|e| panic!("{e}"));
    assert!(!install.is_installed(), "a partial model");
}

#[test]
fn only_snapdragon_8_gen_2_runs_the_precompiled_model() {
    use super::install::supported_soc;
    assert!(supported_soc("SM8550"));
    assert!(supported_soc(" sm8550 "));
    assert!(!supported_soc("SM8650"), "another NPU generation");
    assert!(!supported_soc("Tensor G3"));
    assert!(!supported_soc(""));
}
