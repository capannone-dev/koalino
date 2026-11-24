<!-- This directory reads who call windows show as speaking, through the platforms' accessibility services (#113). -->

# Accessibility adapters

These adapters implement `ports::call_window::CallWindowReader`. During a recording the
`speaker-name-watcher` thread (`core/bin/speaker_name_watcher.rs`) asks them every 2 s which names
call windows show as speaking. The readings become a `SpeakingTimeline`, stored with the meeting.
The voice gallery later uses them to suggest names for unknown voices (`application::cluster_review`).

| File | Purpose |
| --- | --- |
| `mod.rs` | `ReadBudget` (3,000 elements and 800 ms per reading, 64 levels deep), `SpeakingNames` (labels to distinct names with `domain::speaker_names::speaking_name`), and `platform_reader`. |
| `atspi.rs` | Linux: AT-SPI over the accessibility D-Bus with zbus (300 ms per call). It also has `enable_accessibility`, which sets `org.a11y.Status.IsEnabled`. |
| `uia.rs` | Windows: UI Automation through the `windows` crate. COM lives only during one reading. |
| `accessibility_tests.rs` | Budget and label tests. |

Both readers only read. They list top-level windows and keep those whose title belongs to a call
app or meeting site (`is_call_window`). They walk those windows within the budget, reading each
element's name; on Linux they also read the text of an unnamed leaf. Nothing is clicked, focused or
changed, and labels and names are never logged.

Browsers and Electron apps on Linux build their accessibility trees only while assistive technology
is active. `enable_accessibility` turns the session's flag on. The bus launcher keeps it in the
desktop settings, but apps that were already running must be restarted once. The CLI's
`call-names [--seconds N] [--enable]` prints what the reader sees.

Verified on deb-gui (Firefox ESR, a local fixture page) for all three label shapes:
- a tile label ("Alice Rossi is speaking");
- a comma list ("Bruno Bianchi, video is off, speaking");
- a text line ("Talking: Carla Neri").

Verified on win11 (Edge, the same page) through UI Automation. The reader must run in the user's
desktop session: over SSH it sees no windows.

A Chromium window (Edge, Chrome, and the WebView2 inside new Teams) that is fully covered or
minimized stops rendering, and its accessibility tree keeps the last labels. The reader then sees
one stale name for as long as the window stays covered. `domain::speaker_names::suggest_names`
therefore drops a name that wins for more than one voice.

New Teams on Windows (a real call, win11, 2026-11-24) shows who is speaking only visually. The
meeting window exposes about 100 elements. The tiles are labelled with name and microphone state
("Myself video, <name>, Unmuted"), and so is the People roster ("<name>, Organizer, External,
Unmuted"). Name, help text, item status, ARIA properties and full description never said
"speaking" during minutes of speech. So the reader finds no names in Teams. For Teams calls in
the browser, names come from the live captions instead (#102). The labels of Zoom and Meet were
not captured. An app whose labels differ gives no readings until `domain::speaker_names` learns
its phrasing.
