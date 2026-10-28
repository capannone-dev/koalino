<!-- Documents typed browser/server API clients for the local Axum daemon. -->

# API client

This directory centralizes typed calls to the local Axum server. Server pages use cached-disabled
fetch calls for current local state; client components reuse the same contract for assignments and
WebSocket URL construction.

`renameMeeting` sends `PATCH /api/meetings/{id}` and throws `MeetingRenameError`, whose `kind`
distinguishes an invalid title (400), an unknown meeting (404), server errors (5xx), other statuses,
and network failures. `meeting-title.ts` validates titles with the same rules as Rust's
`MeetingTitle::parse` (Issue #42).

`getLogLevel`, `setLogLevel`, and `fetchLogExport` (Issue #46) reach the Settings routes and throw
`LogSettingsError`, whose `kind` is `invalid_level`, `no_logs`, `unavailable` (503), `server`, or
`network`. `fetchLogExport` returns the file and the name from its `Content-Disposition`.

`p2p-api.ts` (Issue #43) is the device sync client: `getP2pStatus` (`GET /api/p2p/status`) and
the pairing steps `makeDiscoverable`, `startPairing` (returns the code), `confirmPairingCode`,
`cancelPairing`, `acceptPairingRequest`, `rejectPairingRequest`, and `removeDevice`. Failures
throw `P2pRequestError`, whose `kind` is `invalid_peer` (400), `not_found` (404), `refused` (409),
`unavailable` (503), `server`, or `network`. It shares `apiBaseUrl` with `api.ts`.
