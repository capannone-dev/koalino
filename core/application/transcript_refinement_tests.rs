//! Specifies how a finished meeting is re-transcribed in long windows and keeps its speakers.

use uuid::Uuid;

use super::transcript_refinement::{
    ChunkSpan, MeetingWord, REFINEMENT_WINDOW_MS, keep_live_where_the_pass_lost_speech,
    plan_windows, reattribute_words,
};
use crate::domain::models::{MeetingId, TranscriptSegment};

fn span(start_ms: u64, end_ms: u64) -> ChunkSpan {
    ChunkSpan { start_ms, end_ms }
}

fn word(text: &str, start_ms: u64, end_ms: u64) -> MeetingWord {
    MeetingWord::new(text, start_ms, end_ms)
}

fn meeting() -> MeetingId {
    MeetingId::new("meeting-a")
}

#[test]
fn windows_join_chunks_up_to_thirty_seconds() {
    let spans = [
        span(0, 10_000),
        span(12_000, 25_000),
        span(26_000, 40_000),
        span(50_000, 95_000),
    ];

    let windows = plan_windows(&spans, REFINEMENT_WINDOW_MS);

    let chunk_lists: Vec<Vec<usize>> = windows
        .iter()
        .map(|window| window.chunk_indexes().to_vec())
        .collect();
    assert_eq!(chunk_lists, vec![vec![0, 1], vec![2], vec![3]]);
    assert_eq!(windows[0].duration_ms(), 23_000);
    assert_eq!(
        windows[2].duration_ms(),
        45_000,
        "a long chunk stands alone"
    );
}

#[test]
fn window_time_maps_back_to_meeting_time_across_the_gap() {
    let windows = plan_windows(
        &[span(0, 10_000), span(12_000, 25_000)],
        REFINEMENT_WINDOW_MS,
    );
    let window = &windows[0];

    assert_eq!(window.to_meeting_ms(4_000), 4_000);
    assert_eq!(window.to_meeting_ms(10_500), 12_500);
    assert_eq!(
        window.to_meeting_ms(99_000),
        25_000,
        "past the end clamps to it"
    );
}

#[test]
fn no_chunks_plan_no_windows() {
    assert!(plan_windows(&[], REFINEMENT_WINDOW_MS).is_empty());
}

#[test]
fn words_keep_the_speaker_of_the_segment_they_fall_in() {
    let person = Uuid::new_v4();
    let unknown = Uuid::new_v4();
    let speakers = vec![
        TranscriptSegment::new(meeting(), 0, 5_000, Some(person), "live a"),
        TranscriptSegment::new(meeting(), 5_000, 9_000, None, "live b")
            .with_unknown_cluster_id(Some(unknown)),
    ];
    let words = [
        word("Ciao", 1_000, 1_400),
        word("a", 1_500, 1_600),
        word("tutti.", 6_000, 6_500),
        word("Bene", 6_600, 7_000),
    ];

    let segments = reattribute_words(&meeting(), &words, &speakers);

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].text(), "Ciao a");
    assert_eq!(segments[0].speaker_identity_id(), Some(person));
    assert_eq!(segments[0].start_time_ms(), 1_000);
    assert_eq!(segments[0].end_time_ms(), 1_600);
    assert_eq!(segments[1].text(), "tutti. Bene");
    assert_eq!(segments[1].unknown_cluster_id(), Some(unknown));
    assert_eq!(segments[1].speaker_identity_id(), None);
}

#[test]
fn a_word_between_segments_takes_the_nearest_speaker() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let speakers = vec![
        TranscriptSegment::new(meeting(), 0, 2_000, Some(first), "a"),
        TranscriptSegment::new(meeting(), 10_000, 12_000, Some(second), "b"),
    ];

    let segments = reattribute_words(
        &meeting(),
        &[word("early", 3_000, 3_200), word("late", 9_000, 9_200)],
        &speakers,
    );

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].speaker_identity_id(), Some(first));
    assert_eq!(segments[1].speaker_identity_id(), Some(second));
}

#[test]
fn a_long_pause_or_a_long_run_starts_a_new_segment() {
    let person = Uuid::new_v4();
    let speakers = vec![TranscriptSegment::new(
        meeting(),
        0,
        90_000,
        Some(person),
        "x",
    )];
    let mut words = vec![word("one", 0, 500), word("two", 5_000, 5_500)];
    words
        .extend((0..30).map(|second| word("on", 10_000 + second * 1_000, 10_400 + second * 1_000)));

    let segments = reattribute_words(&meeting(), &words, &speakers);

    assert_eq!(segments[0].text(), "one", "a 4.5 s pause splits");
    assert!(segments.len() >= 4, "a 30 s run is split as well");
    assert!(
        segments
            .iter()
            .all(|segment| segment.end_time_ms() - segment.start_time_ms() <= 20_000)
    );
}

#[test]
fn without_speakers_the_words_stay_unattributed() {
    let segments = reattribute_words(&meeting(), &[word("hello", 0, 300)], &[]);

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].speaker_identity_id(), None);
    assert_eq!(segments[0].unknown_cluster_id(), None);
}

#[test]
fn blank_words_are_dropped() {
    let segments = reattribute_words(&meeting(), &[word("  ", 0, 300)], &[]);

    assert!(segments.is_empty());
}

mod whole_meeting {
    use std::{cell::RefCell, collections::HashMap};

    use super::super::transcript_refinement::{
        ChunkSpan, RefinementChunk, RefinementError, refine_meeting,
    };
    use crate::{
        adapters::storage::sqlite_adapter::SqliteNoteRepository,
        domain::{
            errors::TranscriptionError,
            models::{
                AudioRecording, DeviceId, MeetingId, RecordingId, TimedWord, Transcript,
                TranscriptSegment, TranscriptionContext,
            },
            transcription_language::TranscriptionLanguage,
        },
        ports::{ai::TranscriptionEngine, storage::NoteRepository},
    };

    /// Answers each window with scripted words and records what it was asked.
    #[derive(Default)]
    struct ScriptedEngine {
        /// Words per call, relative to the window, in call order.
        answers: Vec<Vec<(&'static str, u64, u64)>>,
        /// Fails the call with this index, if set.
        failing_call: Option<usize>,
        /// Previous text and language given with each call.
        contexts: RefCell<Vec<(Option<String>, Option<TranscriptionLanguage>)>>,
        /// Sample counts given to `provide`, keyed by recording id.
        audio: RefCell<HashMap<String, usize>>,
        /// Sample counts the engine read, in call order.
        audio_read: RefCell<Vec<usize>>,
    }

    impl TranscriptionEngine for ScriptedEngine {
        fn transcribe(
            &self,
            audio_recording: &AudioRecording,
            context: &TranscriptionContext,
        ) -> Result<Transcript, TranscriptionError> {
            let call = self.contexts.borrow().len();
            self.contexts.borrow_mut().push((
                context.previous_text().map(str::to_owned),
                context.language(),
            ));
            let read = self
                .audio
                .borrow_mut()
                .remove(audio_recording.recording_id().as_str())
                .ok_or(TranscriptionError::Unavailable)?;
            self.audio_read.borrow_mut().push(read);
            if self.failing_call == Some(call) {
                return Err(TranscriptionError::Unavailable);
            }
            let words = self.answers.get(call).cloned().unwrap_or_default();
            // A word written `?word` is one the engine was unsure of.
            let spelled = |text: &'static str| text.trim_start_matches('?');
            let content = words
                .iter()
                .map(|word| spelled(word.0))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(
                Transcript::new(audio_recording.meeting_id().clone(), content).with_timed_words(
                    words
                        .into_iter()
                        .map(|(text, start, end)| {
                            let probability = if text.starts_with('?') { 0.1 } else { 0.9 };
                            TimedWord::new(spelled(text), start, end).with_probability(probability)
                        })
                        .collect(),
                ),
            )
        }
    }

    fn repository_with(segments: &[(u64, u64, &str)]) -> (SqliteNoteRepository, MeetingId) {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let meeting = MeetingId::new("meeting-r");
        repository
            .create_meeting(&meeting, &[])
            .unwrap_or_else(|error| panic!("{error}"));
        let segments: Vec<TranscriptSegment> = segments
            .iter()
            .map(|(start, end, text)| {
                TranscriptSegment::new(meeting.clone(), *start, *end, None, *text)
            })
            .collect();
        repository
            .save_transcript_segments(&meeting, &segments)
            .unwrap_or_else(|error| panic!("{error}"));
        (repository, meeting)
    }

    fn texts(repository: &SqliteNoteRepository, meeting: &MeetingId) -> Vec<String> {
        repository
            .load_transcript_segments(meeting)
            .unwrap_or_else(|error| panic!("{error}"))
            .iter()
            .map(|segment| segment.text().to_owned())
            .collect()
    }

    /// Two chunks of 1 s and 2 s at 0 s and 5 s, and one of 40 s at 10 s.
    fn chunks() -> Vec<(ChunkSpan, Vec<i16>)> {
        vec![
            (
                ChunkSpan {
                    start_ms: 0,
                    end_ms: 1_000,
                },
                vec![1; 16_000],
            ),
            (
                ChunkSpan {
                    start_ms: 5_000,
                    end_ms: 7_000,
                },
                vec![2; 32_000],
            ),
            (
                ChunkSpan {
                    start_ms: 10_000,
                    end_ms: 50_000,
                },
                vec![3; 640_000],
            ),
        ]
    }

    fn run(
        engine: &ScriptedEngine,
        repository: &SqliteNoteRepository,
        meeting: &MeetingId,
        cancel_after: Option<usize>,
    ) -> Result<super::super::transcript_refinement::RefinementOutcome, RefinementError> {
        let chunks = chunks();
        let borrowed: Vec<RefinementChunk<'_>> = chunks
            .iter()
            .map(|(span, samples)| RefinementChunk {
                span: *span,
                language: Some(TranscriptionLanguage::Code("it")),
                samples,
            })
            .collect();
        let provide = |id: &RecordingId, samples: Vec<i16>| {
            engine
                .audio
                .borrow_mut()
                .insert(id.as_str().to_owned(), samples.len());
        };
        let cancelled =
            || cancel_after.is_some_and(|calls| engine.contexts.borrow().len() >= calls);
        refine_meeting(
            meeting,
            &borrowed,
            engine,
            &provide,
            repository,
            &DeviceId::new("device-a"),
            &cancelled,
        )
    }

    #[test]
    fn a_speaker_without_a_review_clip_gets_one_cut_from_the_retained_speech() {
        use crate::domain::models::UnassignedVoiceClip;
        let (repository, meeting) = repository_with(&[]);
        let (with_none, with_clip, too_short) = (
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        );
        let spoken = |start, end, text, cluster| {
            TranscriptSegment::new(meeting.clone(), start, end, None, text)
                .with_unknown_cluster_id(Some(cluster))
        };
        repository
            .save_transcript_segments(
                &meeting,
                &[
                    spoken(10_000, 14_000, "live a", with_none),
                    spoken(20_000, 23_000, "live b", with_clip),
                    spoken(30_000, 30_400, "live c", too_short),
                ],
            )
            .unwrap_or_else(|error| panic!("{error}"));
        let earlier_clip = uuid::Uuid::new_v4();
        repository
            .save_unassigned_clip(
                UnassignedVoiceClip::new(earlier_clip, meeting.clone(), 0, 1_000, vec![1, 2])
                    .with_cluster_id(Some(with_clip)),
            )
            .unwrap_or_else(|error| panic!("{error}"));
        let engine = ScriptedEngine {
            answers: vec![
                Vec::new(),
                vec![
                    ("Ciao", 500, 1_200),
                    ("a", 1_300, 1_800),
                    ("tutti", 1_900, 2_600),
                    ("quanti.", 2_700, 3_500),
                    ("Sì", 10_200, 12_800),
                    ("ok", 20_050, 20_350),
                ],
            ],
            ..ScriptedEngine::default()
        };

        run(&engine, &repository, &meeting, None).unwrap_or_else(|error| panic!("{error}"));

        let clips = repository
            .get_unassigned_clips(&meeting)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            clips.len(),
            2,
            "one new clip, for the speaker that had none"
        );
        let new_clip = clips
            .iter()
            .find(|clip| clip.cluster_id() == Some(with_none))
            .unwrap_or_else(|| panic!("no clip for the speaker without one"));
        assert_eq!(new_clip.start_time_ms(), 0);
        assert_eq!(new_clip.end_time_ms(), 3_000);
        // 3 s of 16 kHz mono 16-bit audio, after a 44-byte WAV header, all from the third chunk.
        assert_eq!(new_clip.audio_bytes().len(), 44 + 3 * 16_000 * 2);
        assert_eq!(&new_clip.audio_bytes()[44..46], &3_i16.to_le_bytes());
        assert!(clips.iter().any(|clip| clip.clip_id() == earlier_clip));
        assert!(
            clips
                .iter()
                .all(|clip| clip.cluster_id() != Some(too_short))
        );
    }

    #[test]
    fn a_clip_in_another_recording_of_the_meeting_counts_too() {
        use super::super::transcript_refinement::add_missing_review_clips;
        use crate::domain::models::UnassignedVoiceClip;
        let (repository, laptop) = repository_with(&[]);
        let phone = MeetingId::new("meeting-phone");
        repository
            .create_meeting(&phone, &[])
            .unwrap_or_else(|error| panic!("{error}"));
        let (clipped_on_phone, unclipped) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        repository
            .save_unassigned_clip(
                UnassignedVoiceClip::new(uuid::Uuid::new_v4(), phone.clone(), 0, 1_000, vec![1])
                    .with_cluster_id(Some(clipped_on_phone)),
            )
            .unwrap_or_else(|error| panic!("{error}"));
        let samples = vec![7_i16; 16_000 * 10];
        // The phone's chunk, moved onto the laptop's clock (it started 2 s later).
        let chunks = [RefinementChunk {
            span: ChunkSpan {
                start_ms: 2_000,
                end_ms: 12_000,
            },
            language: None,
            samples: &samples,
        }];
        let timeline = [
            TranscriptSegment::new(laptop.clone(), 3_000, 5_000, None, "a")
                .with_unknown_cluster_id(Some(clipped_on_phone)),
            TranscriptSegment::new(laptop.clone(), 6_000, 8_500, None, "b")
                .with_unknown_cluster_id(Some(unclipped)),
        ];

        add_missing_review_clips(&laptop, &[&laptop, &phone], &chunks, &timeline, &repository);

        let clips = repository
            .get_unassigned_clips(&laptop)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].cluster_id(), Some(unclipped));
        assert_eq!(clips[0].end_time_ms(), 2_500);
        assert_eq!(&clips[0].audio_bytes()[44..46], &7_i16.to_le_bytes());
    }

    #[test]
    fn the_new_text_replaces_the_live_text_inside_the_retained_audio() {
        let (repository, meeting) = repository_with(&[
            (0, 900, "live one"),
            (5_000, 6_900, "live two"),
            (10_000, 49_000, "live three"),
            (60_000, 61_000, "before retention"),
        ]);
        let engine = ScriptedEngine {
            answers: vec![
                vec![
                    ("Buongiorno", 100, 600),
                    ("a", 1_200, 1_400),
                    ("tutti.", 1_500, 2_000),
                ],
                vec![("Bene.", 500, 900)],
            ],
            ..ScriptedEngine::default()
        };

        let outcome =
            run(&engine, &repository, &meeting, None).unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(outcome.window_count, 2);
        assert_eq!(
            texts(&repository, &meeting),
            vec!["Buongiorno", "a tutti.", "Bene.", "before retention"],
            "words map back to meeting time; text outside the retained audio stays"
        );
        assert_eq!(*engine.audio_read.borrow(), vec![48_000, 640_000]);
        let contexts = engine.contexts.borrow();
        assert_eq!(contexts[0], (None, Some(TranscriptionLanguage::Code("it"))));
        assert_eq!(
            contexts[1].0.as_deref(),
            Some("Buongiorno a tutti."),
            "each window is prompted with the previous one's text"
        );
    }

    /// Verifies the computer's sound, which the pass does not re-transcribe, stays (#108).
    #[test]
    fn the_computer_sound_inside_the_retained_audio_stays() {
        use crate::domain::models::SegmentSource;
        let (repository, meeting) = repository_with(&[(0, 900, "live one")]);
        let mut segments = repository
            .load_transcript_segments(&meeting)
            .unwrap_or_else(|error| panic!("{error}"));
        segments.push(
            TranscriptSegment::new(meeting.clone(), 200, 800, None, "their words")
                .with_source(SegmentSource::ComputerSound),
        );
        repository
            .save_transcript_segments(&meeting, &segments)
            .unwrap_or_else(|error| panic!("{error}"));
        let engine = ScriptedEngine {
            answers: vec![vec![("Buongiorno", 100, 600)], vec![]],
            ..ScriptedEngine::default()
        };

        run(&engine, &repository, &meeting, None).unwrap_or_else(|error| panic!("{error}"));

        let refined = repository
            .load_transcript_segments(&meeting)
            .unwrap_or_else(|error| panic!("{error}"));
        let heard: Vec<&str> = refined
            .iter()
            .filter(|segment| segment.source() == SegmentSource::ComputerSound)
            .map(TranscriptSegment::text)
            .collect();
        assert_eq!(heard, vec!["their words"]);
        assert!(refined.iter().any(|segment| segment.text() == "Buongiorno"));
        assert!(!refined.iter().any(|segment| segment.text() == "live one"));
    }

    /// Verifies the pass drops the microphone's copy of the computer's sound once echo was heard
    /// in the meeting, and keeps it otherwise (#108).
    #[test]
    fn the_new_text_loses_its_echoes_when_the_meeting_heard_echo() {
        use crate::domain::models::SegmentSource;
        for echo_heard in [false, true] {
            let repository = SqliteNoteRepository::open_in_memory(
                crate::application::crypto::voice_encryptor::ephemeral_cipher(),
            )
            .and_then(|repository| {
                repository.with_document_codec(std::sync::Arc::new(
                    crate::application::meeting_document::AutomergeMeetingDocumentCodec,
                ))
            })
            .unwrap_or_else(|error| panic!("{error}"));
            let meeting = MeetingId::new("meeting-echo");
            repository
                .create_meeting(&meeting, &automerge::AutoCommit::new().save())
                .unwrap_or_else(|error| panic!("{error}"));
            repository
                .save_transcript_segments(
                    &meeting,
                    &[TranscriptSegment::new(
                        meeting.clone(),
                        0,
                        2_000,
                        None,
                        "buongiorno a tutti quanti voi",
                    )
                    .with_source(SegmentSource::ComputerSound)],
                )
                .unwrap_or_else(|error| panic!("{error}"));
            if echo_heard {
                repository
                    .record_echo_heard(&meeting)
                    .unwrap_or_else(|error| panic!("{error}"));
            }
            let engine = ScriptedEngine {
                answers: vec![
                    vec![
                        ("Buongiorno", 100, 400),
                        ("a", 400, 500),
                        ("tutti", 500, 700),
                        ("quanti", 700, 800),
                        ("voi.", 800, 950),
                    ],
                    vec![],
                ],
                ..ScriptedEngine::default()
            };

            run(&engine, &repository, &meeting, None).unwrap_or_else(|error| panic!("{error}"));

            let microphone = repository
                .load_transcript_segments(&meeting)
                .unwrap_or_else(|error| panic!("{error}"))
                .into_iter()
                .filter(|segment| segment.source() == SegmentSource::Microphone)
                .count();
            assert_eq!(
                microphone,
                usize::from(!echo_heard),
                "echo heard: {echo_heard}"
            );
        }
    }

    /// Verifies the pass marks uncertain words, the overlap kept with the meeting, and words far
    /// from any speaker (#90, #91).
    #[test]
    fn the_new_segments_carry_uncertain_words_overlap_and_doubtful_speakers() {
        use crate::{
            application::meeting_document::AutomergeMeetingDocumentCodec,
            domain::transcript_confidence::OverlapSpan,
        };
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .and_then(|repository| {
            repository.with_document_codec(std::sync::Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"));
        let meeting = MeetingId::new("meeting-marks");
        repository
            .create_meeting(&meeting, &automerge::AutoCommit::new().save())
            .unwrap_or_else(|error| panic!("{error}"));
        let speaker = Some(uuid::Uuid::from_u128(7));
        repository
            .save_transcript_segments(
                &meeting,
                &[TranscriptSegment::new(
                    meeting.clone(),
                    0,
                    1_000,
                    speaker,
                    "live",
                )],
            )
            .unwrap_or_else(|error| panic!("{error}"));
        repository
            .record_overlap_spans(&meeting, &[OverlapSpan::new(10_000, 12_000)])
            .unwrap_or_else(|error| panic!("{error}"));
        let engine = ScriptedEngine {
            answers: vec![
                vec![("Buongiorno", 100, 600), ("?tutti.", 700, 900)],
                vec![("Parliamo", 500, 1_500), ("insieme.", 1_600, 2_400)],
            ],
            ..ScriptedEngine::default()
        };

        run(&engine, &repository, &meeting, None).unwrap_or_else(|error| panic!("{error}"));

        let segments = repository
            .load_transcript_segments(&meeting)
            .unwrap_or_else(|error| panic!("{error}"));
        let first = &segments[0];
        assert_eq!(first.text(), "Buongiorno tutti.");
        assert_eq!(first.confidence().uncertain_words, [1]);
        assert!(!first.confidence().uncertain_speaker);
        assert!(!first.confidence().overlapping_speech);
        let later = segments
            .iter()
            .find(|segment| segment.text().starts_with("Parliamo"))
            .unwrap_or_else(|| panic!("the 40 s chunk's words must be written"));
        assert!(
            later.confidence().overlapping_speech,
            "it lies in the kept overlap"
        );
        assert!(
            later.confidence().uncertain_speaker,
            "no speaker was heard within seconds of it"
        );
    }

    #[test]
    fn a_failed_window_leaves_the_live_transcript() {
        let (repository, meeting) = repository_with(&[(0, 900, "live one")]);
        let engine = ScriptedEngine {
            answers: vec![vec![("nuovo", 0, 500)]],
            failing_call: Some(1),
            ..ScriptedEngine::default()
        };

        let result = run(&engine, &repository, &meeting, None);

        assert!(matches!(result, Err(RefinementError::Transcription(_))));
        assert_eq!(texts(&repository, &meeting), vec!["live one"]);
    }

    #[test]
    fn a_cancelled_pass_leaves_the_live_transcript() {
        let (repository, meeting) = repository_with(&[(0, 900, "live one")]);
        let engine = ScriptedEngine {
            answers: vec![vec![("nuovo", 0, 500)]],
            ..ScriptedEngine::default()
        };

        let result = run(&engine, &repository, &meeting, Some(1));

        assert!(matches!(result, Err(RefinementError::Cancelled)));
        assert_eq!(texts(&repository, &meeting), vec!["live one"]);
    }
}

#[test]
fn a_word_stretched_over_a_silence_keeps_its_own_speaker_and_length() {
    // Whisper often gives a word every millisecond up to the next one, so a word before a long
    // silence seems to last through it. Only its first second counts.
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let speakers = vec![
        TranscriptSegment::new(meeting(), 0, 2_000, Some(first), "live a"),
        TranscriptSegment::new(meeting(), 5_000, 7_000, Some(second), "live b"),
    ];
    let words = [word("Ciao", 0, 500), word("sì.", 1_500, 7_000)];

    let segments = reattribute_words(&meeting(), &words, &speakers);

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].speaker_identity_id(), Some(first));
    assert_eq!(segments[0].end_time_ms(), 2_500);
    assert_eq!(segments[0].text(), "Ciao sì.");
}

/// Words spread evenly over `[start_ms, end_ms)`.
fn spoken(text: &str, start_ms: u64, end_ms: u64) -> Vec<MeetingWord> {
    let texts: Vec<&str> = text.split_whitespace().collect();
    let step = (end_ms - start_ms) / texts.len() as u64;
    texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let start = start_ms + index as u64 * step;
            word(text, start, start + step)
        })
        .collect()
}

/// A live microphone segment.
fn live(text: &str, start_ms: u64, end_ms: u64) -> TranscriptSegment {
    TranscriptSegment::new(meeting(), start_ms, end_ms, None, text)
}

fn texts(words: &[MeetingWord]) -> String {
    words
        .iter()
        .map(MeetingWord::text)
        .collect::<Vec<_>>()
        .join(" ")
}

const TURN_ONE: &str = "se non si ha l'abitudine di guidare su strade di campagna";
const TURN_TWO: &str = "questo consentirà ai giocatori di controllare le azioni nel gioco";
const TURN_THREE: &str = "l'attività svolta è stata per lo più teorica ma il programma";

/// Verifies a window keeps the live text when the pass skipped a sentence the live transcript had
/// and wrote the next sentence over it (#88: seen on a real two-device test).
#[test]
fn a_window_that_lost_a_live_sentence_keeps_the_live_text() {
    let spans = [span(0, 10_000), span(11_000, 20_000), span(21_000, 29_000)];
    let mut pass = spoken(TURN_ONE, 0, 10_000);
    // The second sentence is skipped; the third is written early, then again in its place.
    pass.extend(spoken(TURN_THREE, 11_000, 20_000));
    pass.extend(spoken(TURN_THREE, 21_000, 29_000));
    let standing = [
        live(TURN_ONE, 0, 10_000),
        live(TURN_TWO, 11_000, 20_000),
        live(TURN_THREE, 21_000, 29_000),
    ];

    let (words, replaced) = keep_live_where_the_pass_lost_speech(pass, &spans, &standing);

    assert_eq!(replaced, 1);
    assert_eq!(
        texts(&words),
        format!("{TURN_ONE} {TURN_TWO} {TURN_THREE}"),
        "each sentence once, in order"
    );
}

/// Verifies a pass that only rewords the live text is kept.
#[test]
fn a_pass_that_rewords_the_live_text_is_kept() {
    let spans = [span(0, 10_000), span(11_000, 20_000)];
    let pass = [
        spoken(
            "se non si ha abitudine di guidare sulle strade di campagna",
            0,
            10_000,
        ),
        spoken(TURN_TWO, 11_000, 20_000),
    ]
    .concat();
    let standing = [
        live(TURN_ONE, 0, 10_000),
        live(
            "questo consentira ai giocatori di controllare azioni nel gioco",
            11_000,
            20_000,
        ),
    ];

    let (words, replaced) = keep_live_where_the_pass_lost_speech(pass.clone(), &spans, &standing);

    assert_eq!(replaced, 0);
    assert_eq!(texts(&words), texts(&pass));
}

/// Verifies a chunk whose live text is too short to judge (a few words) never vetoes the pass.
#[test]
fn a_short_live_text_does_not_veto_the_pass() {
    let spans = [span(0, 3_000)];
    let pass = spoken("grazie a tutti", 0, 3_000);
    let standing = [live("sottotitoli a cura di", 0, 3_000)];

    let (words, replaced) = keep_live_where_the_pass_lost_speech(pass.clone(), &spans, &standing);

    assert_eq!(replaced, 0);
    assert_eq!(texts(&words), texts(&pass));
}

/// Verifies only the chunk that lost speech takes the live text: the pass's better text of the
/// other chunks in the same window stays (a live chunk may even be in the wrong language).
#[test]
fn only_the_chunk_that_lost_speech_takes_the_live_text() {
    let spans = [span(0, 10_000), span(11_000, 20_000)];
    let pass = spoken(TURN_ONE, 0, 10_000);
    let standing = [
        live(
            "se eu está de tracoi bambi em anciani são state liberados",
            0,
            10_000,
        ),
        live(TURN_TWO, 11_000, 20_000),
    ];

    let (words, replaced) = keep_live_where_the_pass_lost_speech(pass, &spans, &standing);

    assert_eq!(replaced, 1);
    assert_eq!(texts(&words), format!("{TURN_ONE} {TURN_TWO}"));
}

/// Verifies a sentence written over a skipped one is caught when the pass then also lost most of
/// it in its own chunk: the misplaced words match the neighbour's live text (seen on #88).
#[test]
fn a_sentence_moved_over_a_skipped_one_is_caught_by_the_neighbours_live_text() {
    let spans = [span(0, 10_000), span(11_000, 20_000)];
    let mut pass = spoken(TURN_THREE, 0, 10_000);
    pass.extend(spoken("programma", 11_000, 20_000));
    let standing = [live(TURN_TWO, 0, 10_000), live(TURN_THREE, 11_000, 20_000)];

    let (words, replaced) = keep_live_where_the_pass_lost_speech(pass, &spans, &standing);

    assert_eq!(replaced, 2);
    assert_eq!(texts(&words), format!("{TURN_TWO} {TURN_THREE}"));
}

/// Splits `text` on spaces, as window words are.
fn split(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// Verifies a window that opens by restating the end of the previous one, as Whisper does with
/// its prompt (seen at a 30 s seam of a real meeting, 2026-11-24), loses the restated words,
/// although Whisper worded them a little differently and the previous window cut the last word.
#[test]
fn a_window_restating_the_previous_ones_end_drops_the_restatement() {
    let previous = split("va bene così. Ok, comunque si guardiamo. Quindi i risultati s");
    let next = split("Ok, comunque sì, guarderò. Quindi i risultati si basano su questi numeri");
    assert_eq!(super::transcript_refinement::restated_lead(&previous, &next), 8);
}

/// Verifies ordinary continuations keep every word: new speech, a short coincidence, or a
/// sentence that only shares a few words with the end of the previous window.
#[test]
fn a_window_continuing_the_previous_one_keeps_its_words() {
    let restated = |previous: &str, next: &str| {
        super::transcript_refinement::restated_lead(&split(previous), &split(next))
    };
    assert_eq!(restated("abbiamo finito la prova", "poi passiamo al modello"), 0);
    assert_eq!(restated("e poi", "e poi andiamo"), 0);
    assert_eq!(
        restated(
            "abbiamo visto i risultati del modello",
            "i risultati del test sono buoni"
        ),
        0
    );
    assert_eq!(restated("", "una frase nuova qui"), 0);
}
