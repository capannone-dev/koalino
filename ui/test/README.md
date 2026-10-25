<!-- This directory holds widget tests that define Flutter behavior before presentation implementation. -->

# Widget tests

Tests inject a `RecordingBridge` fake into the controller, proving that widgets make state changes
without loading a native Rust library or accessing Rust implementation types.
