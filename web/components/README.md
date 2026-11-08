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
right-click) offering **Rename**. `meeting-title-header.tsx` renders a meeting detail page's
heading with a pencil button. Both open `rename-meeting-dialog.tsx`, the shared modal that
validates the title locally, calls `renameMeeting`, and maps failures to plain messages; they
replace the title from the response without reloading (Issue #42).

`transcription-route-settings.tsx` (Issue #59) chooses where recordings are transcribed.

`glossary-settings.tsx` edits the transcription glossary, one name or term per line, through
`PUT /api/settings/glossary`; a refused list stays in the field with a plain reason.

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
- `meeting-recorded-on.tsx` says "Recorded on Laptop + Pixel" on the page of a meeting the app
  combined from several devices' recordings (#88), and its Split button lists the other recordings
  on their own again after an inline confirmation. The list rows carry the same line.
- `meeting-project-picker.tsx` is the meeting page's Project selector; a failed move puts the
  previous choice back.
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
or "Not connected".
