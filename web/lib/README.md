<!-- Documents typed browser/server API clients for the local Axum daemon. -->

# API client

`i18n/` holds the dashboard's translations (#124); see `i18n/README.md`. Helpers here that word
text for the owner (error messages, meeting metadata, title rules) take a `t: Translate` argument,
which defaults to English.

This directory centralizes typed calls to the local Axum server. Server pages use cached-disabled
fetch calls for current local state; client components reuse the same contract for assignments and
WebSocket URL construction.

`renameMeeting` sends `PATCH /api/meetings/{id}` and throws `MeetingRenameError`, whose `kind`
distinguishes an invalid title (400), an unknown meeting (404), server errors (5xx), other statuses,
and network failures. `meeting-title.ts` validates titles with the same rules as Rust's
`MeetingTitle::parse` (Issue #42). `meeting-metadata.ts` words the meeting page's creation time
(`YYYY-MM-DD HH:MM UTC`, as the apps show it) and transcription languages (language names stay English, as in the
app) with `Intl.DisplayNames`.

`getLogLevel`, `setLogLevel`, and `fetchLogExport` (Issue #46) reach the Settings routes and throw
`LogSettingsError`, whose `kind` is `invalid_level`, `no_logs`, `unavailable` (503), `server`, or
`network`. `fetchLogExport` returns the file and the name from its `Content-Disposition`.

`p2p-api.ts` (Issue #43) is the device sync client: `getP2pStatus` (`GET /api/p2p/status`) and
the pairing steps `makeDiscoverable`, `startPairing` (returns the code), `confirmPairingCode`,
`cancelPairing`, `acceptPairingRequest`, `rejectPairingRequest`, and `removeDevice`, plus
`setDeviceName` (`PUT /api/p2p/device-name`, #121), which returns the saved name. A
`NearbyDevice` may carry the `device_name` it told while discoverable. Failures
throw `P2pRequestError`, whose `kind` is `invalid_peer` (400), `not_found` (404), `refused` (409),
`unavailable` (503), `server`, or `network`. It shares `apiBaseUrl` with `api.ts`.
`DeviceCapabilities` is the manifest summary the status carries for this server and for each
connected paired device (`null` while not connected).

`pass-api.ts` (#87) reads and changes the after-meeting pass: `getAfterMeetingPass` and
`setAfterMeetingPass` over `/api/settings/after-meeting-pass`, throwing `PassError` with
`invalid_model`, `unavailable`, `server` or `network`.

`projects-api.ts` also nests projects (#142): `createProject(name, parentId)`, `moveProject`,
and the pure `projectTree` (each level by name, unknown parents at the top) and `isWithinProject`.

`recordings-api.ts` (#140) reads and changes the kept meeting recordings: `getRecordingsStatus`,
`setKeepRecordings`, `wipeRecordings`, `hasMeetingRecording` and `meetingRecordingUrl` (the WAV
an `<audio>` element plays), throwing `RecordingsError` with the daemon's `error_code`.

`projects-api.ts` (Issue #85) is the projects client: `getProjects`, `getProject`, `createProject`,
`renameProject`, `deleteProject`, `moveMeetingToProject`, `uploadProjectFile` (the raw file, its
name in the query), `addProjectNote`, `deleteProjectResource`, `getProjectEmail`,
`getProjectResourceText`, and `projectResourceFileUrl` for pictures and downloads. Failures throw
`ProjectApiError` with the daemon's `error_code` as `kind` (`unavailable` for 503, `network`,
otherwise `unexpected`); `projectErrorMessage` turns it into a plain message, never the server's
text. `projectNameError` checks names with Rust's rules. `MeetingSummary.project_id` (in `api.ts`)
is the meeting's project, absent from older servers.

`edits-api.ts` (#96) edits a meeting: `getMeetingNotes`, `saveMeetingNotes`, `editSummaryText`
and `editSegment`, with `EditApiError` and `editErrorMessage`. `llm-api.ts` (#95) holds the AI
connections and summaries, and `fetchSummaryExport` downloads a summary as a file (#130).

`search-api.ts` (#100) is `searchNotes(query, scope, projectId?)` over `GET /api/search`, with
`SearchApiError` and `searchErrorMessage` (never the server's text). `hitHref` gives the page and
anchor a hit opens, and `hitPlaceLabel` says where in its meeting or project it is.

`chat-api.ts` (#97) is `getChat`, `askChat` and `clearChat` over `/api/chat`, with
`ChatApiError` and `chatErrorMessage` (never the server's text). `sourceHref` and `sourceLabel`
turn a cited `ChatSource` (#110) into its link and its text.

`meeting-voices-api.ts` (#103) is `getMeetingVoices` and `setMeetingVoices(keep)` over
`/api/settings/meeting-voices`, with `MeetingVoicesError` (`unavailable`, `server`, `network`).

`notes-folder-api.ts` (#98) is `getNotesFolder`, `setNotesFolder(enabled, path)` and
`syncNotesFolder` over `/api/settings/notes-folder`, with `NotesFolderError` and
`notesFolderMessage` (never the server's text).
