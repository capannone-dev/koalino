<!-- Documents the meeting index and detail App Router route tree. -->

# Meeting routes

The index fetches meeting IDs from Axum and the dynamic route fetches one transcript and linked
attachment metadata. No local storage is accessed by these pages.
Both pages hand the fetched titles to client components (`MeetingList`, `MeetingTitleHeader`) that
rename meetings in place (Issue #42).
