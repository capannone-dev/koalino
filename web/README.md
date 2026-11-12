<!-- Explains the Next.js dashboard and its integration with the local Axum daemon. -->

# Koalino Linux Server dashboard

This Next.js App Router application is the local web dashboard for the Rust headless daemon. It
consumes only documented Axum JSON/WebSocket boundaries; it never reads SQLite or local media files
directly.

## Setup and development

Install Node.js 22 or later, then run:

```bash
cd web
npm install
NEXT_PUBLIC_API_BASE_URL=http://127.0.0.1:8080 npm run dev
```

`NEXT_PUBLIC_API_BASE_URL` defaults to `http://127.0.0.1:8080`. The browser WebSocket URL can be
overridden with `NEXT_PUBLIC_WS_URL`; otherwise it derives from the API URL. Run `npm run lint`,
`npm run test`, and `npm run build` before deployment. GitHub Actions (`web-ci.yml`) runs the same
checks on every change under `web/`, so they are verified even where Node.js is unavailable.

## Pages

- `/settings/telemetry` (Settings → Telemetry; Issue #53) is the responsive telemetry dashboard.
  `/dashboard`, its old address, redirects there. It renders device health, summary cards,
  duration, latency, task mix, and power charts from the local Axum API. Its WebSocket client
  refreshes a telemetry update at most once every ten seconds and reconnects after disconnect.
- `/` shows live Axum WebSocket status messages.
- `/meetings` lists local meetings; `/meetings/<id>` shows transcript and attachment metadata.
  Both can rename a meeting (see below).
- `/voice-gallery` loads every meeting's unresolved unknown-speaker clusters from
  `GET /api/meetings/{id}/unknown-clusters`. It shows each cluster's total speech time and chunk
  count, and posts the typed name to `POST /api/clusters/{id}/assign`. An assigned cluster is
  removed from the list.
- `/projects` lists projects and creates them; `/projects/<id>` shows a project's meetings, files,
  notes and e-mails, with uploads, notes, previews and deletion (Issue #85, `app/projects/README.md`).
  A meeting's page has a Project selector.
- `/settings` (Issue #46) links to Telemetry, shows the daemon's log level, changes it until the daemon restarts,
  and downloads the newest week of logs (`app/settings/README.md`).

Cluster responses deliberately carry no audio paths, so the web gallery has no playback. Local
review clips can be played from the Flutter Voice Gallery, which receives local file paths over FFI.

## Renaming meetings (Issue #42)

- **Where:** each `/meetings` row has a "⋯" actions button (44 px, so it works on touch), and
  right-clicking a row opens the same menu. Its **Rename** item opens the rename dialog. On
  `/meetings/<id>`, the pencil beside the title opens the same dialog.
- **Dialog:** `components/rename-meeting-dialog.tsx` is a modal (`role="dialog"`,
  `aria-modal`). The current title is pre-filled and selected. Enter saves; Escape, **Cancel**, or
  a click on the backdrop closes it; Tab stays inside it; focus returns to the button that opened
  it. It is a bottom sheet on phones and a centered `max-w-md` card from the `sm` breakpoint.
- **Validation:** `lib/meeting-title.ts` applies Rust's `MeetingTitle::parse` rules before any
  request: trim, then reject empty titles ("Enter a title."), titles over 200 characters (counted
  in code points, so an emoji counts once), and control characters such as newlines and tabs.
  Errors show after the first Save, then update as the person types. An unchanged title closes the
  dialog without a request.
- **Deleting:** the row menu, the meeting header, unknown voices and saved people offer Delete through
  `ConfirmDeleteDialog`. A refusal stays in the dialog with a plain message.
- **Request:** `renameMeeting` sends `PATCH /api/meetings/{id}` with `{"title": ...}` and maps
  failures to a `MeetingRenameError` kind (400, 404, 5xx, other, network). The dialog stays open
  with a plain message per kind and never shows the server's error text.
- **Result:** the list row or the heading is replaced from the `200` response without reloading the
  page, and a polite status region announces "Meeting renamed".
