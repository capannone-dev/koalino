<!-- This directory contains use-case orchestration that coordinates domain types through ports. -->

# Application layer

Application services implement workflows such as recording processing and compute selection. They contain no direct infrastructure integrations. `compute_router.rs` selects an injected `TranscriptionEngine` port from portable local/remote `DeviceManifest` values; concrete model and encrypted-P2P implementations remain outside the application layer.

`network_events.rs` consumes only `NetworkEvent` values supplied by a `P2pNetwork` port and keeps
an application-visible reachability view. It has no dependency on libp2p identities, swarm events,
or socket APIs.
