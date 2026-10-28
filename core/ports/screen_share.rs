//! Declares local screen-share detection and capture contracts (Issue #49).
//!
//! No operating system reliably reports that another app is sharing the screen, so a share is
//! detected by an adapter for the conferencing app (`ScreenShareDetector`). While a share is
//! active, `ScreenCapturer` grabs frames, the application layer decides which frames are new
//! slides, and `MeetingSlideSink` stores them on the meeting being recorded. Frames are screen
//! contents: they are never logged.

use crate::domain::{
    errors::{ScreenShareError, StorageError},
    models::MeetingId,
};

/// A screen share that a conferencing app is running.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveScreenShare {
    /// A non-sensitive label of the sharing app, e.g. `teams`; never a window title.
    application: String,
}

impl ActiveScreenShare {
    /// Describes a running share.
    ///
    /// Inputs: The sharing app's label. Outputs: The share. Side effects: None.
    pub fn new(application: impl Into<String>) -> Self {
        Self {
            application: application.into(),
        }
    }

    /// Returns the sharing app's label.
    pub fn application(&self) -> &str {
        &self.application
    }
}

/// One captured screen frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenFrame {
    width: usize,
    height: usize,
    /// One 8-bit luma (brightness) value per pixel, row by row; used to compare frames.
    luma: Vec<u8>,
    /// The same frame encoded as PNG or JPEG; stored when the frame is a new slide.
    encoded_image: Vec<u8>,
}

impl ScreenFrame {
    /// Creates a frame after checking its dimensions.
    ///
    /// Inputs: The width and height in pixels (both non-zero), `width * height` luma values, and
    /// the encoded image.
    /// Outputs: The frame, or `InvalidFrame` for empty dimensions or a luma length mismatch.
    /// Side effects: None; the bytes are never logged.
    pub fn new(
        width: usize,
        height: usize,
        luma: Vec<u8>,
        encoded_image: Vec<u8>,
    ) -> Result<Self, ScreenShareError> {
        if width == 0 || height == 0 {
            return Err(ScreenShareError::InvalidFrame {
                reason: String::from("frame dimensions must be non-zero"),
            });
        }
        if width.checked_mul(height) != Some(luma.len()) {
            return Err(ScreenShareError::InvalidFrame {
                reason: format!("expected {width}x{height} luma values, got {}", luma.len()),
            });
        }
        Ok(Self {
            width,
            height,
            luma,
            encoded_image,
        })
    }

    /// Returns the width in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Borrows the luma values, row by row.
    pub fn luma(&self) -> &[u8] {
        &self.luma
    }

    /// Borrows the encoded image (PNG or JPEG).
    pub fn encoded_image(&self) -> &[u8] {
        &self.encoded_image
    }
}

/// Reports whether a conferencing app is sharing the screen; implemented per app.
pub trait ScreenShareDetector {
    /// Checks for a running share.
    ///
    /// Inputs: None.
    /// Outputs: The running share, `None` when nothing is shared, or a detection error.
    /// Side effects: May query the operating system or the conferencing app.
    fn active_share(&self) -> Result<Option<ActiveScreenShare>, ScreenShareError>;
}

/// Grabs the shared screen.
pub trait ScreenCapturer {
    /// Captures one frame of what is being shared.
    ///
    /// Inputs: The running share, so an adapter can capture only the shared window or monitor.
    /// Outputs: The frame, or a capture error.
    /// Side effects: Reads the screen; must not log or keep the frame.
    fn capture_frame(&self, share: &ActiveScreenShare) -> Result<ScreenFrame, ScreenShareError>;
}

/// Stores slides on a meeting.
pub trait MeetingSlideSink {
    /// Attaches one slide image to a meeting.
    ///
    /// Inputs: The meeting (which must exist), the PNG or JPEG image, and the meeting time in
    /// milliseconds at which the slide was shown.
    /// Outputs: Success, or a storage error (unsupported format, file system, missing meeting).
    /// Side effects: Writes the image and an attachment row; never logs the image.
    fn attach_slide(
        &self,
        meeting_id: &MeetingId,
        encoded_image: &[u8],
        meeting_time_ms: u64,
    ) -> Result<(), StorageError>;
}
