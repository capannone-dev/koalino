//! Shows the offer to record a call as a Windows toast (#111) and waits for the owner's answer.
//!
//! The toast says "`<app>` is using the microphone" with Record and Not now. It uses the
//! `reminder` scenario, so it stays on screen until answered, and makes no sound. It comes from
//! the AppUserModelID `Koalino.NoteTaker`, registered under `HKCU\Software\Classes\
//! AppUserModelId` with the name Koalino on first use (no Start menu shortcut is needed). The
//! answer arrives through the toast's in-process events while the app runs. Windows PowerShell
//! cannot subscribe to those events, which is why the core shows it instead of a script.
//!
//! The XML and the answers are portable and tested everywhere; showing a toast exists only on
//! Windows, and elsewhere `offer` answers `Unavailable`.

use std::time::Duration;

/// The AppUserModelID the toasts come from.
pub const APP_ID: &str = "Koalino.NoteTaker";

/// How long an unanswered offer waits before it counts as dismissed.
pub const OFFER_TIMEOUT: Duration = Duration::from_secs(120);

/// What the owner did with the toast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallToastAnswer {
    /// Pressed Record.
    Record,
    /// Pressed Not now, closed it, or it expired or was withdrawn.
    Dismissed,
    /// Clicked the toast itself: no answer, so the in-app banner keeps offering.
    Open,
    /// No toast could be shown; the banner alone offers.
    Unavailable,
}

impl CallToastAnswer {
    /// Reads the activation arguments of the toast's buttons.
    ///
    /// Inputs: `record`, `dismiss`, or anything else (a click on the toast's body gives an empty
    /// string). Outputs: The answer. Side effects: None.
    pub fn from_arguments(arguments: &str) -> Self {
        match arguments {
            "record" => Self::Record,
            "dismiss" => Self::Dismissed,
            _ => Self::Open,
        }
    }

    /// The answer's name for the app: `record`, `dismissed`, `open` or `unavailable`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Dismissed => "dismissed",
            Self::Open => "open",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The toast's XML for `app`'s offer.
///
/// Inputs: The call app's name, escaped here. Outputs: The toast XML. Side effects: None.
pub fn call_toast_xml(app: &str) -> String {
    format!(
        concat!(
            r#"<toast scenario="reminder"><visual><binding template="ToastGeneric">"#,
            "<text>{} is using the microphone</text><text>Record this call?</text>",
            "</binding></visual><actions>",
            r#"<action content="Record" arguments="record" activationType="foreground"/>"#,
            r#"<action content="Not now" arguments="dismiss" activationType="foreground"/>"#,
            r#"</actions><audio silent="true"/></toast>"#
        ),
        escape_xml(app)
    )
}

/// Escapes text for XML content and attributes.
fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Shows the offer and waits for the answer.
///
/// Inputs: The call app's name. Outputs: The answer; `Dismissed` after `OFFER_TIMEOUT` or a
/// `withdraw`, `Unavailable` when no toast could be shown (always, outside Windows).
/// Side effects: On Windows, registers the AppUserModelID once, shows a toast, replacing one
/// still shown, and blocks the calling thread until answered: call it off the UI thread.
pub fn offer(app: &str) -> CallToastAnswer {
    #[cfg(windows)]
    {
        windows_toast::offer(app, OFFER_TIMEOUT).unwrap_or_else(|error| {
            tracing::warn!(error = %error, "The call offer's notification could not be shown");
            CallToastAnswer::Unavailable
        })
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        CallToastAnswer::Unavailable
    }
}

/// Removes the toast shown, if any; its waiting `offer` answers `Dismissed`.
///
/// Side effects: On Windows, hides the toast. Elsewhere, nothing.
pub fn withdraw() {
    #[cfg(windows)]
    windows_toast::withdraw();
}

/// The toast through WinRT (`Windows.UI.Notifications`).
#[cfg(windows)]
mod windows_toast {
    use std::{
        sync::{Mutex, mpsc},
        time::Duration,
    };

    use windows::{
        Data::Xml::Dom::XmlDocument,
        Foundation::TypedEventHandler,
        UI::Notifications::{
            ToastActivatedEventArgs, ToastDismissedEventArgs, ToastFailedEventArgs,
            ToastNotification, ToastNotificationManager, ToastNotifier,
        },
        Win32::System::{
            Com::{COINIT_MULTITHREADED, CoInitializeEx},
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
                RegCreateKeyExW, RegSetValueExW,
            },
        },
        core::{HSTRING, IInspectable, Interface, PCWSTR},
    };

    use super::{APP_ID, CallToastAnswer, call_toast_xml};

    /// The toast on screen and how to answer its waiting `offer`.
    struct Shown {
        notifier: ToastNotifier,
        toast: ToastNotification,
        answer: mpsc::Sender<CallToastAnswer>,
    }

    /// At most one offer is shown.
    static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);

    /// Shows the toast and waits for its answer. Errors are WinRT or registry failures.
    pub(super) fn offer(app: &str, timeout: Duration) -> windows::core::Result<CallToastAnswer> {
        // WinRT needs COM on this thread; a thread already initialized keeps its apartment.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        register_app_id()?;
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(call_toast_xml(app)))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        toast.SetTag(&HSTRING::from("call-offer"))?;
        toast.SetGroup(&HSTRING::from("koalino"))?;
        let (sender, receiver) = mpsc::channel();

        let activated = sender.clone();
        toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, arguments| {
                let chosen = arguments
                    .as_ref()
                    .and_then(|arguments| arguments.cast::<ToastActivatedEventArgs>().ok())
                    .and_then(|arguments| arguments.Arguments().ok())
                    .map(|arguments| arguments.to_string())
                    .unwrap_or_default();
                let _ = activated.send(CallToastAnswer::from_arguments(&chosen));
                Ok(())
            },
        ))?;
        let dismissed = sender.clone();
        toast.Dismissed(&TypedEventHandler::<
            ToastNotification,
            ToastDismissedEventArgs,
        >::new(move |_, _| {
            let _ = dismissed.send(CallToastAnswer::Dismissed);
            Ok(())
        }))?;
        let failed = sender.clone();
        toast.Failed(
            &TypedEventHandler::<ToastNotification, ToastFailedEventArgs>::new(move |_, _| {
                let _ = failed.send(CallToastAnswer::Unavailable);
                Ok(())
            }),
        )?;

        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?;
        withdraw();
        notifier.Show(&toast)?;
        if let Ok(mut shown) = SHOWN.lock() {
            *shown = Some(Shown {
                notifier: notifier.clone(),
                toast: toast.clone(),
                answer: sender,
            });
        }

        let answer = receiver
            .recv_timeout(timeout)
            .unwrap_or(CallToastAnswer::Dismissed);
        // Clears the slot only if it still holds this toast (a newer offer may have replaced it).
        if let Ok(mut shown) = SHOWN.lock()
            && shown.as_ref().is_some_and(|shown| shown.toast == toast)
        {
            *shown = None;
            let _ = notifier.Hide(&toast);
        }
        Ok(answer)
    }

    /// Hides the toast shown and answers its `offer` with `Dismissed`.
    pub(super) fn withdraw() {
        let taken = SHOWN.lock().ok().and_then(|mut shown| shown.take());
        if let Some(shown) = taken {
            let _ = shown.notifier.Hide(&shown.toast);
            let _ = shown.answer.send(CallToastAnswer::Dismissed);
        }
    }

    /// Registers the AppUserModelID with the name Koalino, so the toast shows and names the app.
    fn register_app_id() -> windows::core::Result<()> {
        let path = HSTRING::from(format!(r"Software\Classes\AppUserModelId\{APP_ID}"));
        let mut key = HKEY::default();
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_WRITE,
                None,
                &mut key,
                None,
            )
            .ok()?;
        }
        let name: Vec<u16> = "Koalino\0".encode_utf16().collect();
        let bytes: Vec<u8> = name.iter().flat_map(|unit| unit.to_le_bytes()).collect();
        let value = HSTRING::from("DisplayName");
        let result =
            unsafe { RegSetValueExW(key, PCWSTR(value.as_ptr()), None, REG_SZ, Some(&bytes)).ok() };
        unsafe {
            let _ = RegCloseKey(key);
        }
        result
    }
}
