//! Reads who is speaking from call windows over AT-SPI, the Linux accessibility bus (#113).
//!
//! The session bus's `org.a11y.Bus` gives the accessibility bus address. On that bus the
//! registry's root lists every accessible app, and each app lists its windows. Only windows whose
//! title belongs to a call app or meeting site are walked, breadth first and within a
//! `ReadBudget`. Each element's `Name` (an ARIA label in a browser) and, for an unnamed leaf, the
//! start of its text are passed to `SpeakingNames`. Every D-Bus call has a short timeout, so a
//! busy app delays one reading at most. Nothing is clicked or focused.
//!
//! Browsers and Electron apps build their accessibility trees only while assistive technology is
//! active. `enable_accessibility` sets `org.a11y.Status.IsEnabled`, which tells them one is; apps
//! started earlier may need a restart to notice.

use std::{collections::VecDeque, time::Duration};

use zbus::{
    blocking::{Connection, connection::Builder},
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};

use super::{MAX_DEPTH, ReadBudget, SpeakingNames};
use crate::{
    domain::speaker_names::is_call_window,
    ports::call_window::{CallWindowError, CallWindowReader},
};

/// The AT-SPI accessible interface.
const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
/// The AT-SPI text interface.
const TEXT: &str = "org.a11y.atspi.Text";
/// The registry, whose root lists the accessible apps.
const REGISTRY: &str = "org.a11y.atspi.Registry";
/// The registry's root object.
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
/// Longest wait for one D-Bus answer.
const CALL_TIMEOUT: Duration = Duration::from_millis(300);
/// Characters of a leaf's text kept for a label; longer text is not a speaking label anyway.
const LEAF_TEXT_CHARS: usize = 120;

/// An element: its owner's bus name and its object path.
type Element = (String, OwnedObjectPath);

/// Reads call windows over AT-SPI; connects on the first reading.
#[derive(Default)]
pub struct AtSpiCallWindowReader {
    connection: Option<Connection>,
}

impl AtSpiCallWindowReader {
    /// Creates a reader. Side effects: None.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the accessibility bus connection, opening it when needed.
    fn connection(&mut self) -> Result<&Connection, CallWindowError> {
        if self.connection.is_none() {
            self.connection = Some(connect().map_err(unavailable)?);
        }
        self.connection
            .as_ref()
            .ok_or_else(|| CallWindowError::Unavailable {
                reason: String::from("no accessibility bus connection"),
            })
    }
}

impl CallWindowReader for AtSpiCallWindowReader {
    /// Reads the speaking names of every open call window.
    ///
    /// Outputs: The names, or `Unavailable` when the accessibility bus cannot be reached (the
    /// connection is then reopened on the next reading). An app that does not answer is skipped.
    /// Side effects: D-Bus calls to the registry and the call apps.
    fn speaking_names(&mut self) -> Result<Vec<String>, CallWindowError> {
        let connection = self.connection()?;
        let applications = match children(connection, &(String::from(REGISTRY), root_path()?)) {
            Ok(applications) => applications,
            Err(error) => {
                self.connection = None;
                return Err(unavailable(error));
            }
        };
        let mut budget = ReadBudget::standard();
        let mut names = SpeakingNames::default();
        for application in applications {
            let Ok(windows) = children(connection, &application) else {
                continue;
            };
            for window in windows {
                if !budget.visit() {
                    return Ok(names.into_names());
                }
                let title = name(connection, &window).unwrap_or_default();
                if is_call_window(&title) {
                    read_window(connection, window, &mut budget, &mut names);
                }
            }
        }
        Ok(names.into_names())
    }
}

/// Walks one call window breadth first, reading labels into `names`.
///
/// Side effects: D-Bus calls, at most one element per budget visit.
fn read_window(
    connection: &Connection,
    window: Element,
    budget: &mut ReadBudget,
    names: &mut SpeakingNames,
) {
    let mut queue: VecDeque<(Element, usize)> = VecDeque::from([(window, 0)]);
    while let Some((element, depth)) = queue.pop_front() {
        if !budget.visit() {
            return;
        }
        let label = name(connection, &element).unwrap_or_default();
        names.read_label(&label);
        let element_children = if depth < MAX_DEPTH {
            children(connection, &element).unwrap_or_default()
        } else {
            Vec::new()
        };
        if element_children.is_empty() && label.is_empty() {
            names.read_label(&leaf_text(connection, &element).unwrap_or_default());
        }
        queue.extend(element_children.into_iter().map(|child| (child, depth + 1)));
    }
}

/// Opens the accessibility bus.
fn connect() -> zbus::Result<Connection> {
    let session = Connection::session()?;
    let reply = session.call_method(
        Some("org.a11y.Bus"),
        "/org/a11y/bus",
        Some("org.a11y.Bus"),
        "GetAddress",
        &(),
    )?;
    let address: String = reply.body().deserialize()?;
    Builder::address(address.as_str())?
        .method_timeout(CALL_TIMEOUT)
        .build()
}

/// Tells apps that assistive technology is active, so browsers expose their pages (#113).
///
/// Outputs: `true` when the flag was off and has just been turned on (apps already running,
/// browsers in particular, must be restarted once to notice), `false` when it was already on, or
/// an error when the session's accessibility bus service is missing.
/// Side effects: Sets `org.a11y.Status.IsEnabled` for the desktop session; the bus launcher keeps
/// it in the desktop settings (`toolkit-accessibility`), so it stays on after a new login.
pub fn enable_accessibility() -> Result<bool, CallWindowError> {
    let session = Connection::session().map_err(unavailable)?;
    let reply = session
        .call_method(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.a11y.Status", "IsEnabled"),
        )
        .map_err(unavailable)?;
    let current: OwnedValue = reply.body().deserialize().map_err(unavailable)?;
    if bool::try_from(current).unwrap_or(false) {
        return Ok(false);
    }
    session
        .call_method(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            Some("org.freedesktop.DBus.Properties"),
            "Set",
            &("org.a11y.Status", "IsEnabled", Value::from(true)),
        )
        .map_err(unavailable)?;
    Ok(true)
}

/// The registry root's path.
fn root_path() -> Result<OwnedObjectPath, CallWindowError> {
    OwnedObjectPath::try_from(ROOT_PATH).map_err(|error| CallWindowError::Unavailable {
        reason: error.to_string(),
    })
}

/// Lists an element's children.
fn children(connection: &Connection, element: &Element) -> zbus::Result<Vec<Element>> {
    let (bus_name, path) = element;
    let reply = connection.call_method(
        Some(bus_name.as_str()),
        path,
        Some(ACCESSIBLE),
        "GetChildren",
        &(),
    )?;
    reply.body().deserialize()
}

/// Reads an element's accessible name.
fn name(connection: &Connection, element: &Element) -> zbus::Result<String> {
    let (bus_name, path) = element;
    let reply = connection.call_method(
        Some(bus_name.as_str()),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &(ACCESSIBLE, "Name"),
    )?;
    let value: OwnedValue = reply.body().deserialize()?;
    Ok(String::try_from(value)?)
}

/// Reads the start of a leaf element's text; elements without text fail.
///
/// The whole text is asked for (end offset -1): Firefox answers an end offset past the text's
/// length with nothing.
fn leaf_text(connection: &Connection, element: &Element) -> zbus::Result<String> {
    let (bus_name, path) = element;
    let reply = connection.call_method(
        Some(bus_name.as_str()),
        path,
        Some(TEXT),
        "GetText",
        &(0_i32, -1_i32),
    )?;
    let text: String = reply.body().deserialize()?;
    Ok(text.chars().take(LEAF_TEXT_CHARS).collect())
}

/// Describes a D-Bus failure without any label.
fn unavailable(error: zbus::Error) -> CallWindowError {
    CallWindowError::Unavailable {
        reason: error.to_string(),
    }
}
