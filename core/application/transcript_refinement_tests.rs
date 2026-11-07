//! Specifies how a finished meeting is re-transcribed in long windows and keeps its speakers.

use uuid::Uuid;

use super::transcript_refinement::{
    ChunkSpan, MeetingWord, REFINEMENT_WINDOW_MS, plan_windows, reattribute_words,
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
            let content = words
                .iter()
                .map(|word| word.0)
                .collect::<Vec<_>>()
                .join(" ");
            Ok(
                Transcript::new(audio_recording.meeting_id().clone(), content).with_timed_words(
                    words
                        .into_iter()
                        .map(|(text, start, end)| TimedWord::new(text, start, end))
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
