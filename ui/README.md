<!-- This directory contains the Flutter client and its adapter-only boundary to the Rust core. -->

# Flutter client

The Flutter client renders application state and delegates native work to `infrastructure/`.
Presentation widgets never import Rust-generated or `dart:ffi` bindings. Controllers depend on
small Dart bridge interfaces, allowing widget tests to use deterministic fakes.

## Voice Gallery

`VoiceGalleryScreen` renders `VoiceGalleryController` state for a meeting's unresolved diarization
clips. The controller depends on `VoiceGalleryBridge`; `RustVoiceGalleryBridge` is the only Dart
layer that calls the Rust C ABI. `note_taker_get_unassigned_clips` returns UTF-8 JSON metadata:
clip UUID, time range, optional weak-match candidate, and an absolute `local_file_path`. It never
returns audio bytes or biometric embeddings. The concrete bridge passes that native path to
`audioplayers`, so platform audio code reads the local temporary review file directly.

Weak-match confirmation and new-identity actions send only clip and identity/name strings back to
Rust. Rust retrieves retained derived embeddings from local storage and delegates variant creation
to `VoiceIdentityManager`; Flutter never receives or constructs biometric data.
