<!-- This directory contains infrastructure implementations of ports and operational cross-cutting concerns. -->

# Adapters layer

Adapters are the only layer allowed to integrate external systems. This layer provides observability,
local audio-capture, storage, and libp2p network adapters; future AI adapters belong in dedicated
subdirectories. The in-memory voice-vector adapter is deliberately lightweight and test-oriented;
it performs cosine similarity locally and must be replaced by an encrypted-at-rest production
adapter without changing the application or domain boundaries.
