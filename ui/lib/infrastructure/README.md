<!-- This directory owns Dart's concrete boundary adapter to the Rust C ABI. -->

# FFI infrastructure

The recording bridge converts Dart calls to primitive C ABI functions and status codes. Rust
domain structs are never represented in Dart or presented to widgets.

`attachment_bridge.dart` wraps the standard `file_picker` plugin behind `AttachmentFilePicker` and
maps its selected local path to the narrow Rust C ABI. Suggestions return as caller-freed JSON;
the bridge deliberately leaves JSON parsing to `AttachmentController`.
