//! Specifies one sampling step of local screen-share capture (Issue #49): nothing is captured
//! without a share, new slides reach the meeting with their meeting time, a new share starts
//! over, a failed save is retried, and the no-op adapters never capture. Fake ports only.

use std::sync::{Arc, Mutex};

use crate::{
    adapters::media::no_screen_share::{NoScreenCapturer, NoScreenShareDetector},
    application::screen_share_monitor::{SampleOutcome, ScreenShareMonitor},
    domain::{
        errors::{ScreenShareError, StorageError},
        models::MeetingId,
    },
    ports::screen_share::{
        ActiveScreenShare, MeetingSlideSink, ScreenCapturer, ScreenFrame, ScreenShareDetector,
    },
};

/// The share the fake detector reports; `None` means nothing is shared.
type SharedShare = Arc<Mutex<Option<ActiveScreenShare>>>;

/// Reports whatever share the test set.
struct FakeDetector(SharedShare);

impl ScreenShareDetector for FakeDetector {
    fn active_share(&self) -> Result<Option<ActiveScreenShare>, ScreenShareError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone())
    }
}

/// Returns a solid frame of the brightness the test set, and counts captures.
#[derive(Clone, Default)]
struct FakeCapturer {
    brightness: Arc<Mutex<u8>>,
    capture_count: Arc<Mutex<usize>>,
}

impl ScreenCapturer for FakeCapturer {
    fn capture_frame(&self, _share: &ActiveScreenShare) -> Result<ScreenFrame, ScreenShareError> {
        *self
            .capture_count
            .lock()
            .unwrap_or_else(|error| error.into_inner()) += 1;
        let brightness = *self
            .brightness
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        ScreenFrame::new(
            64,
            36,
            vec![brightness; 64 * 36],
            vec![0x89, b'P', brightness],
        )
    }
}

/// One attached slide: meeting ID, image bytes and meeting time.
type AttachedSlide = (String, Vec<u8>, u64);

/// Records the slides it is given, or fails while `fails` is set.
#[derive(Clone, Default)]
struct FakeSink {
    slides: Arc<Mutex<Vec<AttachedSlide>>>,
    fails: Arc<Mutex<bool>>,
}

impl MeetingSlideSink for FakeSink {
    fn attach_slide(
        &self,
        meeting_id: &MeetingId,
        encoded_image: &[u8],
        meeting_time_ms: u64,
    ) -> Result<(), StorageError> {
        if *self.fails.lock().unwrap_or_else(|error| error.into_inner()) {
            return Err(StorageError::Unavailable);
        }
        self.slides
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push((
                meeting_id.as_str().to_owned(),
                encoded_image.to_vec(),
                meeting_time_ms,
            ));
        Ok(())
    }
}

/// A monitor over fakes, with handles to drive them.
struct Harness {
    monitor: ScreenShareMonitor,
    share: SharedShare,
    capturer: FakeCapturer,
    sink: FakeSink,
}

impl Harness {
    fn new() -> Self {
        let share = SharedShare::default();
        let capturer = FakeCapturer::default();
        let sink = FakeSink::default();
        let monitor = ScreenShareMonitor::new(
            Box::new(FakeDetector(share.clone())),
            Box::new(capturer.clone()),
            Box::new(sink.clone()),
        );
        Self {
            monitor,
            share,
            capturer,
            sink,
        }
    }

    fn share_from(&self, application: Option<&str>) {
        *self.share.lock().unwrap_or_else(|error| error.into_inner()) =
            application.map(ActiveScreenShare::new);
    }

    fn show(&self, brightness: u8) {
        *self
            .capturer
            .brightness
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = brightness;
    }

    fn sample(&mut self, meeting_time_ms: u64) -> SampleOutcome {
        self.monitor
            .sample(&MeetingId::new("meeting-1"), meeting_time_ms)
            .unwrap_or_else(|error| panic!("sample should succeed: {error}"))
    }

    fn saved_times(&self) -> Vec<u64> {
        self.sink
            .slides
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .map(|(_, _, meeting_time_ms)| *meeting_time_ms)
            .collect()
    }
}

/// Verifies nothing is captured while no share runs.
#[test]
fn nothing_is_captured_without_a_share() {
    let mut harness = Harness::new();

    assert_eq!(harness.sample(0), SampleOutcome::NotSharing);
    assert_eq!(harness.sample(2_000), SampleOutcome::NotSharing);
    assert_eq!(
        *harness
            .capturer
            .capture_count
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
        0
    );
}

/// Verifies a stable slide reaches the active meeting with the meeting time it was saved at.
#[test]
fn a_stable_slide_is_attached_to_the_meeting_with_its_meeting_time() {
    let mut harness = Harness::new();
    harness.share_from(Some("teams"));
    harness.show(200);

    assert_eq!(harness.sample(4_000), SampleOutcome::NoNewSlide);
    assert_eq!(harness.sample(6_000), SampleOutcome::SlideSaved);
    assert_eq!(harness.sample(8_000), SampleOutcome::NoNewSlide);

    let slides = harness
        .sink
        .slides
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    assert_eq!(
        slides,
        vec![(String::from("meeting-1"), vec![0x89, b'P', 200], 6_000)]
    );
}

/// Verifies a share that ends and starts again saves its first slide again.
#[test]
fn a_new_share_starts_over() {
    let mut harness = Harness::new();
    harness.share_from(Some("teams"));
    harness.show(200);
    harness.sample(0);
    harness.sample(2_000);

    harness.share_from(None);
    assert_eq!(harness.sample(4_000), SampleOutcome::NotSharing);
    harness.share_from(Some("teams"));
    harness.sample(6_000);
    harness.sample(8_000);

    assert_eq!(harness.saved_times(), vec![2_000, 8_000]);
}

/// Verifies switching to another app's share starts over too.
#[test]
fn a_share_from_another_app_starts_over() {
    let mut harness = Harness::new();
    harness.share_from(Some("teams"));
    harness.show(200);
    harness.sample(0);
    harness.sample(2_000);

    harness.share_from(Some("zoom"));
    harness.sample(4_000);
    harness.sample(6_000);

    assert_eq!(harness.saved_times(), vec![2_000, 6_000]);
}

/// Verifies a slide that could not be saved is saved at a later sample.
#[test]
fn a_failed_save_is_retried() {
    let mut harness = Harness::new();
    harness.share_from(Some("teams"));
    harness.show(200);
    *harness
        .sink
        .fails
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = true;
    harness.sample(0);
    let failed = harness.monitor.sample(&MeetingId::new("meeting-1"), 2_000);
    assert!(matches!(failed, Err(ScreenShareError::Storage(_))));

    *harness
        .sink
        .fails
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = false;
    harness.sample(4_000);
    harness.sample(6_000);

    assert_eq!(harness.saved_times(), vec![6_000]);
}

/// Verifies the no-op adapters never report a share, so nothing is ever captured.
#[test]
fn the_no_op_adapters_never_capture() {
    let sink = FakeSink::default();
    let mut monitor = ScreenShareMonitor::new(
        Box::new(NoScreenShareDetector),
        Box::new(NoScreenCapturer),
        Box::new(sink.clone()),
    );

    for meeting_time_ms in [0, 2_000, 4_000] {
        let outcome = monitor
            .sample(&MeetingId::new("meeting-1"), meeting_time_ms)
            .unwrap_or_else(|error| panic!("no-op detection should succeed: {error}"));
        assert_eq!(outcome, SampleOutcome::NotSharing);
    }
    assert!(
        sink.slides
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty()
    );
    assert!(matches!(
        NoScreenCapturer.capture_frame(&ActiveScreenShare::new("teams")),
        Err(ScreenShareError::Unavailable)
    ));
}
