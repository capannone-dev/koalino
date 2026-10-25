<!-- This directory contains the Flutter client and its adapter-only boundary to the Rust core. -->

# Flutter client

The Flutter client renders application state and delegates native work to `infrastructure/`.
Presentation widgets never import Rust-generated or `dart:ffi` bindings. Controllers depend on
small Dart bridge interfaces, allowing widget tests to use deterministic fakes.
