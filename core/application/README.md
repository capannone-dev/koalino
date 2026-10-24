<!-- This directory contains use-case orchestration that coordinates domain types through ports. -->

# Application layer

Application services implement workflows such as recording processing and compute selection. They contain no direct infrastructure integrations. `compute_router.rs` selects an injected `TranscriptionEngine` port from portable local/remote `DeviceManifest` values; concrete model and encrypted-P2P implementations remain outside the application layer.
