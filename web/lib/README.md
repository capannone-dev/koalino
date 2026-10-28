<!-- Documents typed browser/server API clients for the local Axum daemon. -->

# API client

This directory centralizes typed calls to the local Axum server. Server pages use cached-disabled
fetch calls for current local state; client components reuse the same contract for assignments and
WebSocket URL construction.

`renameMeeting` sends `PATCH /api/meetings/{id}` and throws `MeetingRenameError`, whose `kind`
distinguishes an invalid title (400), an unknown meeting (404), server errors (5xx), other statuses,
and network failures. `meeting-title.ts` validates titles with the same rules as Rust's
`MeetingTitle::parse` (Issue #42).
