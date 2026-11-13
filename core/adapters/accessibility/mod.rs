//! Reads who is speaking from call windows' accessibility trees (#113).
//!
//! `atspi` (Linux, over the AT-SPI D-Bus with zbus) and `uia` (Windows, UI Automation) implement
//! `ports::call_window::CallWindowReader`. Both only read: they list top-level windows, keep those
//! whose title belongs to a call app or meeting site (`domain::speaker_names::is_call_window`),
//! walk each one within `ReadBudget`, and turn element labels into names with
//! `domain::speaker_names::speaking_name`. Labels are never logged.

use std::time::{Duration, Instant};

use crate::{domain::speaker_names::speaking_name, ports::call_window::CallWindowReader};

#[cfg(test)]
mod accessibility_tests;
#[cfg(target_os = "linux")]
pub mod atspi;
#[cfg(windows)]
pub mod uia;

/// The most elements one reading visits, over all call windows.
pub const MAX_NODES_PER_READING: usize = 3_000;
/// The longest one reading may take.
pub const MAX_READING_TIME: Duration = Duration::from_millis(800);
/// The deepest element visited below a window.
pub const MAX_DEPTH: usize = 64;

/// Limits one reading's work, so a huge or slow tree cannot stall the watcher.
#[derive(Debug)]
pub struct ReadBudget {
    /// When the reading must stop.
    deadline: Instant,
    /// Elements that may still be visited.
    nodes_left: usize,
}

impl ReadBudget {
    /// Starts a budget of `MAX_NODES_PER_READING` elements and `MAX_READING_TIME`.
    pub fn standard() -> Self {
        Self::new(MAX_NODES_PER_READING, MAX_READING_TIME)
    }

    /// Starts a budget. Inputs: The element count and the time allowed. Side effects: None.
    pub fn new(nodes: usize, time: Duration) -> Self {
        Self {
            deadline: Instant::now() + time,
            nodes_left: nodes,
        }
    }

    /// Spends one element. Outputs: `false` once the elements or the time ran out.
    pub fn visit(&mut self) -> bool {
        if self.nodes_left == 0 || Instant::now() >= self.deadline {
            return false;
        }
        self.nodes_left -= 1;
        true
    }
}

/// Collects the distinct speaking names found in labels.
#[derive(Debug, Default)]
pub struct SpeakingNames {
    names: Vec<String>,
}

impl SpeakingNames {
    /// Adds the name a label shows as speaking, if any.
    ///
    /// Inputs: One element label. Side effects: Remembers a new name once.
    pub fn read_label(&mut self, label: &str) {
        if label.is_empty() {
            return;
        }
        if let Some(name) = speaking_name(label)
            && !self.names.contains(&name)
        {
            self.names.push(name);
        }
    }

    /// Returns the names in the order found.
    pub fn into_names(self) -> Vec<String> {
        self.names
    }
}

/// Creates this platform's reader.
///
/// Outputs: The AT-SPI reader on Linux, the UI Automation reader on Windows, `None` elsewhere.
/// Side effects: None until the first reading.
pub fn platform_reader() -> Option<Box<dyn CallWindowReader>> {
    #[cfg(target_os = "linux")]
    {
        Some(Box::new(atspi::AtSpiCallWindowReader::new()))
    }
    #[cfg(windows)]
    {
        Some(Box::new(uia::UiAutomationCallWindowReader::new()))
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}
