<!-- This directory contains the Dart-package recording state controller. -->

# Controllers

`RecordingController` maps bridge success or failure into displayable recording state. It contains
no `dart:ffi` code and can be exercised through an in-memory bridge fake.

`AttachmentController` owns selected-file workflow state and decodes the Rust-owned JSON array of
meeting candidates. It receives both a picker and `AttachmentBridge` interface, preserving
widget-test injection and ensuring no complex Rust struct crosses FFI.
