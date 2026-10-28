//! Samples the shared screen while recording and stores new slides on the meeting (Issue #49).
//!
//! The caller (the recording's screen-share watcher) calls `sample` at a regular interval with
//! the meeting being recorded and the current meeting time. Each step asks the
//! `ScreenShareDetector` whether a conferencing app is sharing; if so it grabs a frame through the
//! `ScreenCapturer`, lets `SlideChangeDetector` decide whether the frame is a new slide, and hands
//! new slides to the `MeetingSlideSink` with the meeting time, so they sit at the right point of
//! the transcript. Without a share, nothing is captured. Frames are never logged or kept.

use crate::{
    application::slide_change_detector::SlideChangeDetector,
    domain::{errors::ScreenShareError, models::MeetingId},
    ports::screen_share::{MeetingSlideSink, ScreenCapturer, ScreenShareDetector},
};

/// What one sampling step did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleOutcome {
    /// No share is running; nothing was captured.
    NotSharing,
    /// A frame was captured but is not a new slide (still changing, or already saved).
    NoNewSlide,
    /// A new slide was attached to the meeting.
    SlideSaved,
}

/// Owns the screen-share ports and the slide-change state of the share being watched.
pub struct ScreenShareMonitor {
    share_detector: Box<dyn ScreenShareDetector + Send>,
    screen_capturer: Box<dyn ScreenCapturer + Send>,
    slide_sink: Box<dyn MeetingSlideSink + Send>,
    change_detector: SlideChangeDetector,
    /// The app whose share was seen at the previous step; `None` when nothing was shared.
    sharing_application: Option<String>,
}

impl ScreenShareMonitor {
    /// Creates a monitor that has seen no share yet.
    ///
    /// Inputs: The detector for the conferencing app, the capturer, and the slide store; all are
    /// `Send` so the monitor can run on the watcher thread.
    /// Outputs: The monitor. Side effects: None.
    pub fn new(
        share_detector: Box<dyn ScreenShareDetector + Send>,
        screen_capturer: Box<dyn ScreenCapturer + Send>,
        slide_sink: Box<dyn MeetingSlideSink + Send>,
    ) -> Self {
        Self {
            share_detector,
            screen_capturer,
            slide_sink,
            change_detector: SlideChangeDetector::new(),
            sharing_application: None,
        }
    }

    /// Runs one sampling step.
    ///
    /// Inputs: The meeting being recorded (it must exist) and the current meeting time in
    /// milliseconds.
    /// Outputs: What the step did, or a detection, capture or storage error. A slide that could
    /// not be stored is offered again once the screen is stable at a later step.
    /// Side effects: Queries the detector; while a share runs, captures one frame and may write
    /// one slide to the meeting. A share that ends, or one from another app, starts over.
    pub fn sample(
        &mut self,
        meeting_id: &MeetingId,
        meeting_time_ms: u64,
    ) -> Result<SampleOutcome, ScreenShareError> {
        let Some(share) = self.share_detector.active_share()? else {
            self.reset();
            return Ok(SampleOutcome::NotSharing);
        };
        if self.sharing_application.as_deref() != Some(share.application()) {
            self.change_detector.reset();
            self.sharing_application = Some(share.application().to_owned());
        }
        let frame = self.screen_capturer.capture_frame(&share)?;
        if !self.change_detector.observe(&frame) {
            return Ok(SampleOutcome::NoNewSlide);
        }
        if let Err(error) =
            self.slide_sink
                .attach_slide(meeting_id, frame.encoded_image(), meeting_time_ms)
        {
            // Forget the slide, so the next stable sample saves it.
            self.change_detector.reset();
            return Err(error.into());
        }
        Ok(SampleOutcome::SlideSaved)
    }

    /// Forgets the share being watched, as at the start of a new recording.
    ///
    /// Inputs: None. Outputs: None. Side effects: Clears the slide-change state.
    pub fn reset(&mut self) {
        self.change_detector.reset();
        self.sharing_application = None;
    }
}
