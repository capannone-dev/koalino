<!-- This directory renders Material 3 recording controls from controller-provided state. -->

# Presentation

`RecordingScreen` observes `RecordingController` and calls its public methods. It has no knowledge
of Rust types, dynamic-library handles, or native status codes.

`AttachmentImportScreen` renders the Meeting Detail paperclip control and the Material 3 dialog
for Rust-ranked meeting suggestions. It depends only on `AttachmentController` and requires an
explicit user selection before the controller calls Rust to persist a link.

`HomeShell` switches between the Record, Meetings, and Telemetry destinations, with a bottom
`NavigationBar` below 840 px and a `NavigationRail` above it. `RecordingScreen` adds an optional,
width-capped meeting title field. `MeetingListScreen` renders titles above their date and
duration, and pushes `MeetingDetailScreen` on phones or shows `MeetingDetailView` beside the list
on desktops. The detail view puts the title in a bold `headlineMedium` semantic header
(Issue #36).
