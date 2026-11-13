//! Notices a call from the apps using the microphone (#111).
//!
//! The desktop asks the system which apps capture the microphone (`MicrophoneUser`, from
//! `adapters::audio::microphone_users`) every few seconds and feeds the answer to `CallWatch`.
//! Android hides which app captures it, so the phone reports a voice call as one unnamed user
//! (`ANDROID_VOICE_CALL`), shown as `UNNAMED_CALL_APP`.
//! A call app (Zoom, Teams, Webex, Slack, Discord, Skype...) or a browser (Meet, Jitsi and Teams
//! run in one) that holds the microphone for `START_MS` starts a call; the call ends once no
//! call app has held it for `END_GRACE_MS`, so muting or switching devices does not end it. The
//! app itself never counts. Nothing here records: the app offers to, and only the owner's tap
//! starts a recording.

/// One app capturing the microphone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MicrophoneUser {
    /// Its executable's name (`zoom`, `ms-teams.exe`) or, for a Windows Store app, its package.
    pub process: String,
    /// The name it gives its stream or itself, possibly empty.
    pub name: String,
    /// Its process ID, when known.
    pub pid: Option<u32>,
}

/// A change the app reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallEvent {
    /// A call app started using the microphone: offer to record.
    Started {
        /// The app's name, for the offer.
        app: &'static str,
    },
    /// The call app stopped using it: a recording started for this call may stop.
    Ended {
        /// The app's name.
        app: &'static str,
    },
}

/// The process name the Android host gives a voice call in progress (another app holds the
/// audio in communication mode, or captures the voice-call source), since it cannot see which app.
pub const ANDROID_VOICE_CALL: &str = "android:voice-communication";
/// The name shown for a call whose app is unknown.
pub const UNNAMED_CALL_APP: &str = "A call app";

/// A call app holding the microphone this long starts a call.
pub const START_MS: u64 = 3_000;
/// The microphone free of call apps this long ends a call.
pub const END_GRACE_MS: u64 = 20_000;

/// Known call apps and browsers: a lowercase fragment of the process or package name, and the
/// name shown. The first match wins, so specific names come before the browsers.
const CALL_APPS: &[(&str, &str)] = &[
    ("zoom", "Zoom"),
    ("teams", "Teams"),
    ("webex", "Webex"),
    ("skype", "Skype"),
    ("slack", "Slack"),
    ("discord", "Discord"),
    ("signal", "Signal"),
    ("telegram", "Telegram"),
    ("whatsapp", "WhatsApp"),
    ("facetime", "FaceTime"),
    ("gotomeeting", "GoTo Meeting"),
    ("firefox", "Firefox"),
    ("msedge", "Edge"),
    ("chromium", "Chromium"),
    ("chrome", "Chrome"),
    ("brave", "Brave"),
    ("vivaldi", "Vivaldi"),
    ("opera", "Opera"),
    (ANDROID_VOICE_CALL, UNNAMED_CALL_APP),
];

/// Names a call app.
///
/// Inputs: The process or package name and the stream's name. Outputs: The app's display name,
/// or `None` for other apps (recorders, dictation, this app). Side effects: None.
pub fn call_app(process: &str, name: &str) -> Option<&'static str> {
    let process = process.to_lowercase();
    if process.contains("note_taker") || process.contains("koalino") {
        return None;
    }
    let name = name.to_lowercase();
    CALL_APPS
        .iter()
        .find(|(fragment, _)| process.contains(fragment) || name.contains(fragment))
        .map(|(_, shown)| *shown)
}

/// Follows the microphone's users over time.
#[derive(Debug, Default)]
pub struct CallWatch {
    /// The call app seen since when, before the call counts as started.
    candidate: Option<(&'static str, u64)>,
    /// The call in progress: its app and when a call app last held the microphone.
    call: Option<(&'static str, u64)>,
}

impl CallWatch {
    /// Creates a watch with no call. Side effects: None.
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes one look at the microphone's users.
    ///
    /// Inputs: The time in milliseconds (any steady clock), the users, and this app's process ID
    /// (its streams never count). Outputs: The event this look causes, if any. Side effects:
    /// None beyond the watch's own state.
    pub fn observe(
        &mut self,
        now_ms: u64,
        users: &[MicrophoneUser],
        own_pid: Option<u32>,
    ) -> Option<CallEvent> {
        let seen = users
            .iter()
            .filter(|user| own_pid.is_none() || user.pid != own_pid)
            .find_map(|user| call_app(&user.process, &user.name));
        match (self.call, seen) {
            (Some((app, _)), Some(_)) => {
                self.call = Some((app, now_ms));
                None
            }
            (Some((app, last_seen)), None) => {
                if now_ms.saturating_sub(last_seen) >= END_GRACE_MS {
                    self.call = None;
                    Some(CallEvent::Ended { app })
                } else {
                    None
                }
            }
            (None, Some(app)) => match self.candidate {
                Some((candidate, since)) if candidate == app => {
                    if now_ms.saturating_sub(since) >= START_MS {
                        self.candidate = None;
                        self.call = Some((app, now_ms));
                        Some(CallEvent::Started { app })
                    } else {
                        None
                    }
                }
                _ => {
                    self.candidate = Some((app, now_ms));
                    None
                }
            },
            (None, None) => {
                self.candidate = None;
                None
            }
        }
    }

    /// The app of the call in progress, if any. Side effects: None.
    pub fn in_call(&self) -> Option<&'static str> {
        self.call.map(|(app, _)| app)
    }
}
