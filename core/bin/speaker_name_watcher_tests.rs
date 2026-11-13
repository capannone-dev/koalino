//! Specifies the call-window reading thread of a recording (#113).

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use super::{SpeakerNameWatcher, SpeakerNamesSetup};
use crate::{
    domain::{errors::StorageError, models::MeetingId},
    ports::call_window::{CallWindowError, CallWindowReader, SpeakerNamesSink},
    screen_share_watcher::RecordingClock,
};

/// Answers a fixed name, or fails when told to.
struct ScriptedReader {
    failing: bool,
}

impl CallWindowReader for ScriptedReader {
    fn speaking_names(&mut self) -> Result<Vec<String>, CallWindowError> {
        if self.failing {
            return Err(CallWindowError::Unavailable {
                reason: String::from("no bus"),
            });
        }
        Ok(vec![String::from("Alice")])
    }
}

/// Keeps what was attached.
#[derive(Clone, Default)]
struct RecordingSink {
    attached: Arc<Mutex<Vec<(String, String, u64)>>>,
}

impl SpeakerNamesSink for RecordingSink {
    fn attach_speaker_names(
        &self,
        meeting_id: &MeetingId,
        webvtt: &str,
        meeting_time_ms: u64,
    ) -> Result<(), StorageError> {
        if let Ok(mut attached) = self.attached.lock() {
            attached.push((
                meeting_id.as_str().to_owned(),
                webvtt.to_owned(),
                meeting_time_ms,
            ));
        }
        Ok(())
    }
}

fn setup(failing: bool, enabled: bool, sink: RecordingSink) -> SpeakerNamesSetup {
    SpeakerNamesSetup {
        reader_factory: Arc::new(move || {
            Some(Box::new(ScriptedReader { failing }) as Box<dyn CallWindowReader>)
        }),
        enabled: Box::new(move || enabled),
        sink: Box::new(sink),
        sample_interval: Duration::from_millis(5),
    }
}

fn run_watcher(setup: &SpeakerNamesSetup, capturing: bool) {
    let clock = Arc::new(RecordingClock::default());
    clock.publish(60_000, capturing);
    let watcher = SpeakerNameWatcher::start(setup, Arc::clone(&clock))
        .expect("test thread starts")
        .expect("reading is on");
    std::thread::sleep(Duration::from_millis(60));
    watcher.finish(setup, &MeetingId::new("meeting-a"), 1_000);
}

#[test]
fn names_read_while_capturing_are_attached_at_stop() {
    let sink = RecordingSink::default();
    let names_setup = setup(false, true, sink.clone());
    run_watcher(&names_setup, true);
    let attached = sink.attached.lock().expect("lock").clone();
    assert_eq!(attached.len(), 1);
    let (meeting, webvtt, meeting_time_ms) = &attached[0];
    assert_eq!(meeting, "meeting-a");
    assert!(webvtt.contains("<v Alice>"), "{webvtt}");
    assert_eq!(*meeting_time_ms, 1_000);
}

#[test]
fn nothing_is_attached_while_paused_or_when_reading_fails() {
    let sink = RecordingSink::default();
    run_watcher(&setup(false, true, sink.clone()), false);
    run_watcher(&setup(true, true, sink.clone()), true);
    assert!(sink.attached.lock().expect("lock").is_empty());
}

#[test]
fn no_watcher_runs_when_the_owner_turned_reading_off() {
    let names_setup = setup(false, false, RecordingSink::default());
    let clock = Arc::new(RecordingClock::default());
    assert!(
        SpeakerNameWatcher::start(&names_setup, clock)
            .expect("no thread error")
            .is_none()
    );
}
