//! Reads who is speaking from call windows with Windows UI Automation (#113).
//!
//! Each reading initializes COM on the calling thread, creates the UI Automation client, and lists
//! the desktop's top-level windows with the control-view walker. Only windows whose title belongs
//! to a call app or meeting site are walked, depth first within a `ReadBudget`; each element's
//! `Name` is passed to `SpeakingNames`. The COM objects live only during one reading, so the
//! reader itself holds nothing thread-bound. Nothing is clicked or focused.

use windows::Win32::{
    System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
    UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
    },
};

use super::{MAX_DEPTH, ReadBudget, SpeakingNames};
use crate::{
    domain::speaker_names::is_call_window,
    ports::call_window::{CallWindowError, CallWindowReader},
};

/// Reads call windows with UI Automation.
#[derive(Default)]
pub struct UiAutomationCallWindowReader;

impl UiAutomationCallWindowReader {
    /// Creates a reader. Side effects: None.
    pub fn new() -> Self {
        Self
    }
}

impl CallWindowReader for UiAutomationCallWindowReader {
    /// Reads the speaking names of every open call window.
    ///
    /// Outputs: The names, or `Unavailable` when UI Automation cannot be created.
    /// Side effects: Initializes COM on this thread (multithreaded) and queries other apps'
    /// UI Automation providers.
    fn speaking_names(&mut self) -> Result<Vec<String>, CallWindowError> {
        // SAFETY: COM initialization has no preconditions; a thread already initialized in
        // another mode returns an error that is ignored, as UI Automation works in either mode.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        // SAFETY: Creates the documented in-process UI Automation client.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
                .map_err(unavailable)?;
        // SAFETY: Plain UI Automation calls on a client created above.
        let (root, walker) = unsafe {
            (
                automation.GetRootElement().map_err(unavailable)?,
                automation.ControlViewWalker().map_err(unavailable)?,
            )
        };
        let mut budget = ReadBudget::standard();
        let mut names = SpeakingNames::default();
        let mut window = first_child(&walker, &root);
        while let Some(current) = window {
            if !budget.visit() {
                break;
            }
            if is_call_window(&element_name(&current)) {
                read_window(&walker, &current, &mut budget, &mut names);
            }
            window = next_sibling(&walker, &current);
        }
        Ok(names.into_names())
    }
}

/// Walks one call window depth first, reading names into `names`.
fn read_window(
    walker: &IUIAutomationTreeWalker,
    window: &IUIAutomationElement,
    budget: &mut ReadBudget,
    names: &mut SpeakingNames,
) {
    let mut stack: Vec<(IUIAutomationElement, usize)> = Vec::new();
    if let Some(child) = first_child(walker, window) {
        stack.push((child, 1));
    }
    while let Some((element, depth)) = stack.pop() {
        if !budget.visit() {
            return;
        }
        names.read_label(&element_name(&element));
        if let Some(sibling) = next_sibling(walker, &element) {
            stack.push((sibling, depth));
        }
        if depth < MAX_DEPTH
            && let Some(child) = first_child(walker, &element)
        {
            stack.push((child, depth + 1));
        }
    }
}

/// Returns an element's first child in the control view.
fn first_child(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
) -> Option<IUIAutomationElement> {
    // SAFETY: A UI Automation call on live COM objects; no child is an error or a null result.
    unsafe { walker.GetFirstChildElement(element) }.ok()
}

/// Returns an element's next sibling in the control view.
fn next_sibling(
    walker: &IUIAutomationTreeWalker,
    element: &IUIAutomationElement,
) -> Option<IUIAutomationElement> {
    // SAFETY: As `first_child`.
    unsafe { walker.GetNextSiblingElement(element) }.ok()
}

/// Reads an element's name, empty when it has none.
fn element_name(element: &IUIAutomationElement) -> String {
    // SAFETY: A UI Automation property read on a live element.
    unsafe { element.CurrentName() }
        .map(|name| name.to_string())
        .unwrap_or_default()
}

/// Describes a COM failure without any label.
fn unavailable(error: windows::core::Error) -> CallWindowError {
    CallWindowError::Unavailable {
        reason: error.to_string(),
    }
}
