<!-- Documents reusable presentational and client-interactive dashboard components. -->

# Dashboard components

Components render API DTOs from `web/lib/api.ts`. Client components are limited to WebSocket status,
the cluster-assignment form, meeting renaming, the log settings, the Devices panel, and the project pages; they do not access databases or Rust implementation details.

`voice-gallery.tsx` renders one card per unknown-speaker cluster with a name field and an
**Assign** button. A blank name is rejected without a request. A successful assignment removes the
card and reports the resolved name. A failed request keeps the card and shows an error message.
Each card names its meeting and has an audio player when a review clip exists; the name field
suggests saved people, who are listed below with their sample counts (Issue #63).

`meeting-list.tsx` renders the meeting index with a "⋯" actions menu per row (also opened by
right-click) offering **Rename**, **Add to project…** (`meeting-project-dialog.tsx`, which loads
the projects and shows the picker, #94), **Summary and prompt…** (#95, a link to the meeting's
summary panel) and **Delete**; each row also has a summary icon linking there. `meeting-title-header.tsx` renders a meeting detail page's
heading with a pencil button. Both open `rename-meeting-dialog.tsx`, the shared modal that
validates the title locally, calls `renameMeeting`, and maps failures to plain messages; they
replace the title from the response without reloading (Issue #42).

`transcription-route-settings.tsx` (Issue #59) chooses where recordings are transcribed.

`glossary-settings.tsx` edits the transcription glossary, one name or term per line, through
`PUT /api/settings/glossary`; a refused list stays in the field with a plain reason.
`ai-settings.tsx` (#95) lists the AI connections (a server on the local network, or Claude Code
and Codex on this computer, or an online service with a write-only API key, #138), adds, edits,
tests and removes them, picks the
default, edits the summary prompt template, turns automatic summaries on or off (#101) and chooses
the connection that writes the summaries paired devices ask for (#107), all through
`lib/llm-api.ts`. `meeting-summary-panel.tsx` shows a meeting's stored summary, writes one
with a chosen connection and edits the meeting's own prompt, and downloads the summary as PDF,
plain text, Word or Markdown (#130).
`meeting-notes.tsx` (#96) shows a meeting's Markdown notes as written and edits them in a text
field (`lib/edits-api.ts`, `PUT /api/meetings/{id}/notes`). `meeting-summary-panel.tsx` also edits
the summary's text, or takes one written by hand ("Written by you."). `meeting-transcript.tsx`,
given the meeting's ID, corrects one segment at a time with a pencil button, keeping its speaker
and timing; emptying a segment removes it, and the status line says the meeting is no longer
re-transcribed.
`search-panel.tsx` (#100) is the search box on the meetings page (every meeting), the projects
page (every project) and a project's page (that project), through `lib/search-api.ts`. Each
result names its meeting or project and where the match is (`1:01 · Anna`, `Summary`,
`In plan.md`), marks the matched words, and links to the passage (`#t-<start ms>` in the
transcript), `#summary`, `#notes`, or the file (`#file-<id>` on the project page). The page
outlines the `:target` it lands on.

`chat-panel.tsx` (#97) is a chat with notes through `lib/chat-api.ts`: on a meeting's page, a
project's page and the Chat page (`app/chat`, every meeting, summary and project). Each answer
says what the model was given, what was left out and which connection answered, and links the
places it cites (#110): a meeting at the cited moment, or a file on its project's page.

`notes-folder-settings.tsx` (#98) is Settings' notes folder on the server's computer: the folder,
a switch that turns it on (writing it at once) or off, Sync now, the last round, and the warning
that the folder is a decrypted copy.
`meeting-voices-settings.tsx` (#103) is the Settings switch *Keep meeting participants' voices*
(off by default) through `PUT /api/settings/meeting-voices`. It explains that rooms are never kept,
and a failed save goes back to the saved choice.

`pass-settings.tsx` turns the after-meeting pass (#87) on or off and chooses its model, from the
models on the server's computer, through `PUT /api/settings/after-meeting-pass`.

`retention-settings.tsx` chooses how many days speech waits to be combined with another device's
recording (#93) through `PUT /api/settings/retention`; a refused choice goes back to the saved one.
Nested projects (#142): `projects-index.tsx` indents subprojects under their parent;
`project-view.tsx` lists a project's subprojects, creates one inside it, and moves the project
(Move to, never into itself or its subprojects); `meeting-project-picker.tsx` indents them too.
`recordings-settings.tsx` (#140) turns Keep meeting recordings on after a privacy and space
warning, shows how many are kept, and wipes them all after a confirmation.
`meeting-recording-player.tsx` (#140, #197) is the bar that plays a meeting's kept recording,
sticking to the bottom of the window while the transcript scrolls: play and pause, back and forward
10 seconds, the position, the speed, a button to follow the transcript again, and a link that
downloads the recording as `<title>.wav` (#199). It and
`meeting-transcript.tsx` share the playback through `lib/recording-playback.tsx`; the transcript
marks the passage being heard, keeps it in view, and plays any passage from its start.

`log-settings.tsx` (Issue #46) renders the Settings page's level selector (a directive set
elsewhere, such as `info,note_taker_core=debug`, shows as "Custom") and **Export logs**, which
downloads the file through `fetchLogExport` and saves it with a temporary link. Failures map to
plain messages; server text is never shown.

`primary-nav.tsx` renders the header links (Meetings, Projects, Voice gallery, Settings).

Projects (Issue #85), over `lib/projects-api.ts`:

- `projects-index.tsx` lists projects with "1 meeting · 2 files" and creates one, checking the
  name as Rust does first, then opens it.
- `project-view.tsx` is a project's page: rename, delete (confirmed), its meetings (open, remove,
  add one outside any project), Add files, Write a note, and each file with View, Open (PDFs),
  Download and Delete (confirmed). Previews never render a file's markup: pictures inline,
  Markdown and text as plain text, e-mails as headers and text. A file still syncing shows
  "Syncing…".
- `meeting-transcript.tsx` renders the meeting page's transcript: one block per segment with the
  speaker and start, `Name (?)` for a guessed speaker, an "Overlapping speech" label, and unclear
  words in italics (#90, #91); an imported meeting shows its flat text. A `#at-<ms>` link (a
  chat source, #110) scrolls to the passage that moment falls in.
- `meeting-recorded-on.tsx` says "Recorded on Laptop + Pixel" on the page of a meeting the app
  combined from several devices' recordings (#88), and its Split button lists the other recordings
  on their own again after an inline confirmation. The list rows carry the same line.
- `meeting-project-picker.tsx` is the meeting page's Project selector; a failed move puts the
  previous choice back. Its last option, "New project…", asks for a name, creates the project
  and moves the meeting into it in one step (#94).
`language-settings.tsx` (#124) is the Settings card choosing the dashboard's language: "Same as the
browser" or one of the ten, kept in this browser's `koalino-language` cookie. A change refreshes the
page, so the server renders it in the new language. Every component reads its text through `useT()`
(`lib/i18n`).

`telemetry-entry.tsx` is the Settings card linking to `/settings/telemetry` (Issue #53).

`telemetry-dashboard.tsx` owns the responsive Recharts visualizations and bounded WebSocket refresh
loop. It receives typed local API DTOs only and never reads SQLite or media files.

`devices-entry.tsx` (Issue #84) is the Settings card that opens the Devices page, which left the primary navigation.
`devices-panel.tsx` (Issue #43) is the Devices page body. It polls the P2P status every two
seconds and offers the same steps and wording as the Flutter Devices screen: Make discoverable
(shown as discoverable for the daemon's five-minute pairing window), Pair with a nearby device,
the six-digit code to compare with **Codes match** or **Cancel pairing**, **Accept** or
**Reject** for requests, and **Remove** for a paired device after an inline confirmation. Failures
map to plain messages; server text is never shown. This server and each paired device show
their manifest in one line (`describeCapabilities`, such as "8 CPU cores · Whisper (on device)"),
or "Not connected". A nearby device shows the name it told, with its short peer ID below
(#121), and **Rename** opens an inline field for this server's name; a refused name stays there
with its reason.

`meeting-time.tsx` (#210) shows a meeting's time in the reader's time zone and words ("Today 9:32 AM"),
formatted after mounting, since the server cannot know the browser's time zone. Meeting rows show it
instead of the meeting's internal ID (#211).

`primary-nav.tsx` (#205) is the primary navigation: links with an icon and a word, the current
section marked (`aria-current`); under 640 px it is a bar fixed to the bottom of the window.
`icons.tsx` (#205, #207) holds the web's icon set, Material icons drawn inline, the same symbols as
the app. `live-status.tsx` shows whether live updates arrive as a dot in the header; the home page
now opens the meetings.

`meeting-tabs.tsx` (#201) gives the meeting page its tabs, Summary · Transcript · Notes · Files (an ARIA tab list
with arrow keys; hidden panels stay mounted, so an unsaved note survives a switch), and `MeetingAskAi`, the button
that opens the chat about the meeting. The page puts the recording player after the tabs, so it stays docked under
each of them.

`meeting-transcript.tsx` also shows a one-time tip (#213) over the transcript: grey italic words are ones Koalino was
unsure of, and the pencil beside a passage corrects it. Dismissal is remembered in this browser's `localStorage`.
