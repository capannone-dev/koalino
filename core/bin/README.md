<!-- This directory contains executable composition roots that wire ports to adapters at runtime. -->

# Binary entry points

`cli.rs` is the command-line composition root and `daemon.rs` is the headless service composition root. They own process-level configuration such as logging. `ffi.rs` owns the C-compatible Flutter boundary: it exposes primitive status-code functions only, keeping Rust domain structs and adapter implementations out of Dart.
