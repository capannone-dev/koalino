<!-- This directory holds widget tests that define Flutter behavior before presentation implementation. -->

# Widget tests

Tests inject a `RecordingBridge` fake into the controller, proving that widgets make state changes
without loading a native Rust library or accessing Rust implementation types.

`attachment_import_screen_test.dart` injects fake picker and FFI boundaries. It specifies that the
paperclip action opens the picker, JSON candidates render in rank order with confidence scores,
and Select forwards the exact opaque attachment and meeting IDs to Rust.
