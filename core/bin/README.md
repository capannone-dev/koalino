<!-- This directory contains executable composition roots that wire ports to adapters at runtime. -->

# Binary entry points

`cli.rs` is the command-line composition root and `daemon.rs` is the headless service composition root. They own process-level configuration such as logging.
