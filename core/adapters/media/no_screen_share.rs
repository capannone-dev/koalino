//! Screen-share adapters that detect and capture nothing (Issue #49).
//!
//! No operating system reliably reports that another app is sharing the screen, so real
//! detection belongs to an adapter for each conferencing app. Until one exists, compositions use
//! these: `NoScreenShareDetector` never reports a share, so `ScreenShareMonitor` never captures,
//! and `NoScreenCapturer` refuses to capture if it is ever asked.

use crate::{
    domain::errors::ScreenShareError,
    ports::screen_share::{ActiveScreenShare, ScreenCapturer, ScreenFrame, ScreenShareDetector},
};

/// Never reports a screen share.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoScreenShareDetector;

impl ScreenShareDetector for NoScreenShareDetector {
    /// Reports that nothing is shared.
    ///
    /// Inputs: None. Outputs: Always `Ok(None)`. Side effects: None.
    fn active_share(&self) -> Result<Option<ActiveScreenShare>, ScreenShareError> {
        Ok(None)
    }
}

/// Captures nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoScreenCapturer;

impl ScreenCapturer for NoScreenCapturer {
    /// Refuses to capture.
    ///
    /// Inputs: Any share. Outputs: Always `Unavailable`. Side effects: None.
    fn capture_frame(&self, _share: &ActiveScreenShare) -> Result<ScreenFrame, ScreenShareError> {
        Err(ScreenShareError::Unavailable)
    }
}
