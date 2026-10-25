<!-- This directory is the Dart package entry point for the Flutter client. -->

# Flutter package sources

The package exposes the application entry point and separates presentation, controller, and FFI
infrastructure code. Dart package imports resolve only from this directory.

Attachment import adds a controller-owned JSON parsing boundary: the concrete picker and Rust FFI
adapter live in `infrastructure`, while `presentation/attachment_import_screen.dart` renders the
paperclip action and confirmation dialog from controller state.
