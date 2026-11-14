//! Specifies the NPU Whisper engine's portable parts (#75): the log-mel input, the vocabulary and
//! mel filters read from a whisper.cpp model file, and the greedy decoder loop over a fake NPU.

use super::{
    ggml_vocab::{GgmlVocabError, WhisperVocab},
    greedy_decoder::{
        DecodeRequest, DecodedToken, EOT, LogitScale, NO_SPEECH, NpuError, SOT, SOT_PREV,
        TIMESTAMP_BEGIN, WhisperNpu, decode_greedy, language_token, segments_of, token_times,
    },
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

/// An NPU that answers each step from a script: at each position, the tokens to rate above the
/// rest (as quantized logits); every other token gets 1,000.
struct ScriptedNpu {
    fed: Vec<(u32, usize)>,
    answers: Vec<Vec<(u32, u16)>>,
    encoded: usize,
    restarted: usize,
}

impl ScriptedNpu {
    fn new(answers: Vec<Vec<(u32, u16)>>) -> Self {
        Self {
            fed: Vec::new(),
            answers,
            encoded: 0,
            restarted: 0,
        }
    }
}

impl WhisperNpu for ScriptedNpu {
    fn encode(&mut self, mel: &[u16]) -> Result<(), NpuError> {
        assert_eq!(mel.len(), N_MELS * N_FRAMES);
        self.encoded += 1;
        Ok(())
    }

    fn restart(&mut self) -> Result<(), NpuError> {
        self.restarted += 1;
        Ok(())
    }

    fn step(&mut self, token: u32, position: usize) -> Result<Vec<u16>, NpuError> {
        self.fed.push((token, position));
        let mut logits = vec![1_000_u16; 51_865];
        // Italian a little ahead of English among the languages.
        logits[language_token("it").unwrap_or(0) as usize] = 50_000;
        logits[language_token("en").unwrap_or(0) as usize] = 40_000;
        if let Some(answers) = self.answers.get(self.fed.len() - 1) {
            for (answer, value) in answers {
                logits[*answer as usize] = *value;
            }
        }
        Ok(logits)
    }
}

/// The logits' quantization in the precompiled model.
const SCALE: LogitScale = LogitScale {
    scale: 0.001_292_54,
    zero_point: 17_867,
};

/// A request with no prompt.
fn request(
    language: Option<&'static str>,
    timestamps: bool,
    max_positions: usize,
) -> DecodeRequest<'static> {
    DecodeRequest {
        language,
        prompt: &[],
        timestamps,
        max_positions,
        logit_scale: SCALE,
    }
}

#[test]
fn a_forced_language_is_decoded_greedily_until_the_end() {
    // Steps: SOT, language, transcribe, no timestamps, then text.
    let mut npu = ScriptedNpu::new(vec![
        vec![],
        vec![],
        vec![],
        vec![(7, 60_000), (50_400, 65_535)],
        vec![(8, 60_000)],
        vec![(EOT, 60_000)],
    ]);
    let decoded = decode_greedy(
        &mut npu,
        &[0; N_MELS * N_FRAMES],
        &request(Some("en"), false, 199),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        decoded.tokens.iter().map(|t| t.token).collect::<Vec<_>>(),
        vec![7, 8],
        "a timestamp rated higher is never picked without timestamps"
    );
    assert!(decoded.complete);
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
    let sure = decoded.tokens[1].probability;
    assert!(
        sure > 0.99,
        "a logit far above the rest is near-certain: {sure}"
    );
}

#[test]
fn without_a_language_the_model_picks_one_and_the_length_is_capped() {
    let five = vec![(5, 60_000)];
    let mut npu = ScriptedNpu::new(vec![
        vec![],
        vec![],
        vec![],
        five.clone(),
        five.clone(),
        five.clone(),
        five.clone(),
        five,
    ]);
    let decoded = decode_greedy(&mut npu, &[0; N_MELS * N_FRAMES], &request(None, false, 8))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decoded.language, "it");
    assert_eq!(npu.fed[1], (language_token("it").unwrap_or(0), 1));
    assert_eq!(decoded.tokens.len(), 4, "8 positions, 4 of them forced");
    assert!(!decoded.complete, "cut by the window");
    assert_eq!(language_token("xx"), None);
    assert_eq!(language_token("it"), Some(50_274));
}

#[test]
fn timestamps_follow_whisper_rules() {
    let ts = |seconds_hundredths: u32| TIMESTAMP_BEGIN + seconds_hundredths / 2;
    // Steps: SOT, language, transcribe, then sampling.
    let mut npu = ScriptedNpu::new(vec![
        vec![],
        vec![],
        // First sampled token: text rated highest, but a window must start with a timestamp,
        // and not later than 1 s.
        vec![(7, 65_000), (ts(300), 64_000), (ts(40), 60_000)],
        // After the first timestamp only text: a timestamp rated highest is refused.
        vec![(ts(100), 65_000), (8, 60_000)],
        // Text then a timestamp closes the segment...
        vec![(ts(240), 60_000)],
        // ...and after a lone timestamp, text is refused: the next must be a timestamp (not
        // earlier than the last) or the end.
        vec![(9, 65_000), (ts(100), 64_000), (ts(240), 60_000)],
        vec![(10, 60_000)],
        vec![(EOT, 60_000)],
    ]);
    let decoded = decode_greedy(
        &mut npu,
        &[0; N_MELS * N_FRAMES],
        &request(Some("it"), true, 199),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        decoded.tokens.iter().map(|t| t.token).collect::<Vec<_>>(),
        vec![ts(40), 8, ts(240), ts(240), 10]
    );
    assert!(
        !npu.fed.iter().any(|(token, _)| *token == 50_363),
        "no <|notimestamps|>"
    );
}

#[test]
fn a_prompt_goes_before_the_transcript_and_language_detection_restarts() {
    let mut npu = ScriptedNpu::new(vec![
        vec![], // language detection on SOT alone
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![(EOT, 60_000)],
    ]);
    let request = DecodeRequest {
        language: None,
        prompt: &[11, 12],
        timestamps: false,
        max_positions: 199,
        logit_scale: SCALE,
    };
    let decoded = decode_greedy(&mut npu, &[0; N_MELS * N_FRAMES], &request)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(decoded.language, "it");
    assert_eq!(npu.restarted, 1, "the detection step is undone");
    let italian = language_token("it").unwrap_or(0);
    assert_eq!(
        npu.fed,
        vec![
            (SOT, 0),
            (SOT_PREV, 0),
            (11, 1),
            (12, 2),
            (SOT, 3),
            (italian, 4),
            (50_359, 5),
            (50_363, 6),
        ]
    );
    assert!(decoded.tokens.is_empty());
}

#[test]
fn segments_and_the_next_window_follow_the_timestamps() {
    let ts = |hundredths: u32| TIMESTAMP_BEGIN + hundredths / 2;
    let token = |token: u32| DecodedToken {
        token,
        probability: 0.9,
    };
    // Two closed segments, then an unfinished third: the next window starts at the last pair.
    let tokens = [ts(0), 7, 8, ts(240), ts(240), 9, ts(500), ts(500), 10].map(token);
    let (segments, consumed) = segments_of(&tokens, true, 480_000);
    assert_eq!(segments.len(), 2);
    assert_eq!((segments[0].start_ms, segments[0].end_ms), (0, 2_400));
    assert_eq!(
        segments[0]
            .tokens
            .iter()
            .map(|t| t.token)
            .collect::<Vec<_>>(),
        vec![7, 8]
    );
    assert_eq!((segments[1].start_ms, segments[1].end_ms), (2_400, 5_000));
    assert_eq!(consumed, 80_000, "5 s at 16 kHz");

    // Ending on a lone timestamp: the whole window is done.
    let ended = [ts(0), 7, ts(240), ts(240), 9, ts(500)].map(token);
    let (segments, consumed) = segments_of(&ended, true, 480_000);
    assert_eq!(segments.len(), 2);
    assert_eq!(consumed, 480_000);

    // No pair at all: one segment for the whole window, which is done.
    let loose = [ts(100), 7, 8].map(token);
    let (segments, consumed) = segments_of(&loose, false, 160_000);
    assert_eq!(segments.len(), 1);
    assert_eq!((segments[0].start_ms, segments[0].end_ms), (1_000, 10_000));
    assert_eq!(consumed, 160_000);
}

#[test]
fn prompt_text_is_tokenized_like_whisper() {
    // IDs are merge ranks: "bc" (3) merges before "ab" (4), so "abc" is a + bc, where a
    // longest-match tokenizer would take ab + c.
    let vocab = WhisperVocab::from_tokens(&["a", "b", "c", "bc", "ab", " ", " a"]);
    assert_eq!(vocab.tokenize("abc"), vec![0, 3]);
    assert_eq!(
        vocab.tokenize("abc abc"),
        vec![0, 3, 6, 3],
        "a word keeps its leading space: \"bc\" merges, then \" a\""
    );
    assert_eq!(
        vocab.tokenize("Zz"),
        Vec::<u32>::new(),
        "unknown bytes are skipped"
    );
}

#[test]
fn combining_marks_start_a_new_word_as_in_gpt2() {
    // GPT-2's split counts only \p{L} as letters. The Thai vowel sign U+0E34 is a mark (Mn), so
    // "a\u{0E34}" is two words and "a" + 0xE0 (rank 4) may not merge across them.
    let vocab = WhisperVocab::from_byte_tokens(&[
        b"a",
        b"\xE0",
        b"\xB8",
        b"\xB4",
        b"a\xE0",
        b"\xE0\xB8",
        b"\xE0\xB8\xB4",
    ]);
    assert_eq!(vocab.tokenize("a\u{0E34}"), vec![0, 6]);
}

#[test]
fn segment_time_is_shared_out_by_length() {
    let times = token_times(&[b" Ciao".as_slice(), b" a", b" tutti"], 1_000, 2_300);
    assert_eq!(
        times,
        vec![(100, 150), (150, 170), (170, 230)],
        "10 ms units"
    );
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
/// `NPU_VOCAB` (a multilingual whisper.cpp model), `NPU_LANGUAGE`, and optionally `NPU_PROMPT`.
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
            &TranscriptionContext::new(std::env::var("NPU_PROMPT").ok()),
        )
        .unwrap_or_else(|e| panic!("{e}"));
    println!(
        "opened in {:?}, transcribed in {:?}: {}",
        started - opened,
        started.elapsed(),
        transcript.content()
    );
    for word in transcript.timed_words() {
        println!(
            "{:>7} {:>7} {:>4} {}",
            word.start_ms(),
            word.end_ms(),
            word.probability_permille().unwrap_or(0),
            word.text()
        );
    }
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

#[test]
fn a_recording_becomes_timed_words_window_by_window() {
    use super::transcriber::{WindowSettings, transcribe_windows};
    let ts = |hundredths: u32| TIMESTAMP_BEGIN + hundredths / 2;
    let vocab = WhisperVocab::from_tokens(&[" Ciao", " a", " tutti", " ancora", "[BLANK_AUDIO]"]);
    // 40 s of audio: the first window ends inside a segment at 20 s, so the second starts there.
    let samples = vec![0.0_f32; 16_000 * 40];
    let mut npu = ScriptedNpu::new(vec![
        // Window 1 (language forced): SOT, language, transcribe, then sampling.
        vec![],
        vec![],
        vec![(ts(0), 60_000)],
        vec![(0, 60_000)],
        vec![(1, 60_000)],
        vec![(2, 60_000)],
        vec![(ts(1300), 60_000)],
        vec![(ts(1300), 60_000)],
        vec![(4, 60_000)],
        vec![(ts(2000), 60_000)],
        vec![(ts(2000), 60_000)],
        vec![(3, 60_000)],
        vec![(EOT, 60_000)],
        // Window 2 (20 s to 40 s), prompted with the first window's text.
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![],
        vec![(ts(0), 60_000)],
        vec![(3, 60_000)],
        vec![(ts(300), 60_000)],
        vec![(EOT, 60_000)],
    ]);
    let settings = WindowSettings {
        language: Some("it"),
        prompt: Vec::new(),
        input_scale: 1.0,
        input_zero_point: 0,
        logit_scale: SCALE,
        max_positions: 199,
    };
    let transcript =
        transcribe_windows(&mut npu, &vocab, &samples, &settings).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        transcript.text, "Ciao a tutti ancora",
        "the marker segment is dropped"
    );
    let words: Vec<(&str, u64, u64)> = transcript
        .words
        .iter()
        .map(|word| (word.text(), word.start_ms(), word.end_ms()))
        .collect();
    assert_eq!(
        words,
        vec![
            ("Ciao", 0, 5_000),
            ("a", 5_000, 7_000),
            ("tutti", 7_000, 13_000),
            ("ancora", 20_000, 23_000),
        ]
    );
    assert!(
        transcript
            .words
            .iter()
            .all(|word| word.probability_permille().is_some())
    );
    assert_eq!(npu.encoded, 2);
    // The second window is prompted with the first's text tokens.
    let second_start = npu
        .fed
        .iter()
        .rposition(|(token, position)| *token == SOT_PREV && *position == 0);
    let second_start = second_start.unwrap_or_else(|| panic!("no prompt in the second window"));
    assert_eq!(
        npu.fed[second_start + 1..second_start + 4]
            .iter()
            .map(|(t, _)| *t)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn a_window_the_model_finds_silent_and_unsure_is_dropped() {
    use super::transcriber::{WindowSettings, transcribe_windows};
    let ts = |hundredths: u32| TIMESTAMP_BEGIN + hundredths / 2;
    let vocab = WhisperVocab::from_tokens(&[" E", " che", " altra"]);
    let samples = vec![0.0_f32; 16_000 * 5];
    // Each word barely ahead of three others: about 28% sure.
    let unsure = |token: u32| vec![(token, 60_100), (10, 60_000), (11, 60_000), (12, 60_000)];
    let mut npu = ScriptedNpu::new(vec![
        // After SOT: <|nospeech|> very likely.
        vec![(NO_SPEECH, 65_000)],
        vec![],
        vec![(ts(0), 60_000)],
        unsure(0),
        unsure(1),
        unsure(2),
        vec![(ts(200), 60_000)],
        vec![(EOT, 60_000)],
    ]);
    let settings = WindowSettings {
        language: Some("it"),
        prompt: Vec::new(),
        input_scale: 1.0,
        input_zero_point: 0,
        logit_scale: SCALE,
        max_positions: 199,
    };
    let transcript =
        transcribe_windows(&mut npu, &vocab, &samples, &settings).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        transcript.text, "",
        "Whisper's no-speech rule drops the made-up words"
    );
    assert!(transcript.words.is_empty());
}

/// Writes the tokens of each line of `NPU_TOKENIZE_IN`, from the vocabulary in `NPU_VOCAB` (a
/// whisper.cpp model), to `NPU_TOKENIZE_OUT` (space-separated IDs per line), to compare with
/// Whisper's own tokenizer.
#[test]
#[ignore = "needs a whisper.cpp model and a text file"]
fn writes_the_tokens_of_each_line() {
    let variable = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is unset"));
    let model = std::fs::File::open(variable("NPU_VOCAB")).unwrap_or_else(|e| panic!("{e}"));
    let vocab =
        WhisperVocab::read(&mut std::io::BufReader::new(model)).unwrap_or_else(|e| panic!("{e}"));
    let text =
        std::fs::read_to_string(variable("NPU_TOKENIZE_IN")).unwrap_or_else(|e| panic!("{e}"));
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            vocab
                .tokenize(line)
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    std::fs::write(variable("NPU_TOKENIZE_OUT"), lines.join("\n"))
        .unwrap_or_else(|e| panic!("{e}"));
}
