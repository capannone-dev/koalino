<!-- Documents local media adapters: attachment metadata and screen-share stand-ins. -->

# Media adapters

This directory contains filesystem-facing implementations of `MetadataExtractor`. These adapters
are composed outside the application layer: `AttachmentManager` receives only the port and never
uses filesystem or file-format APIs directly.

`local_file_metadata.rs` currently returns the local file's modification time as its UTC capture
time. Platform EXIF/PDF-specific adapters can replace it later without changing attachment
suggestion policy or Flutter's JSON boundary.

`no_screen_share.rs` (Issue #49) holds the stand-in screen-share adapters:

- `NoScreenShareDetector` never reports a share, so `ScreenShareMonitor` never captures;
- `NoScreenCapturer` refuses to capture (`Unavailable`).

No operating system reliably tells an app that another app is sharing the screen, so real
detection will be one adapter per conferencing app, implementing `ScreenShareDetector`. A capturer
for the platform's screen API implements `ScreenCapturer`.
