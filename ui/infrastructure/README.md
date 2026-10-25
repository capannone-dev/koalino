<!-- This directory contains the Dart FFI adapter used to invoke the Rust C ABI. -->

# Infrastructure

`RustRecordingBridge` is the sole Flutter-side owner of `dart:ffi` calls for recording and logging.
It maps C-compatible status codes into Dart outcomes; no Rust struct crosses this boundary.
