<!-- This directory contains infrastructure implementations of ports and operational cross-cutting concerns. -->

# Adapters layer

Adapters are the only layer allowed to integrate external systems. This layer provides observability,
local audio-capture, attachment metadata, storage, libp2p network adapters, and opt-in local AI
adapters. `ai/local_whisper_cpp.rs` is compiled only with `--features local-ai`; it implements
`TranscriptionEngine` without allowing whisper.cpp types into the application layer. The in-memory
voice-vector adapter is deliberately lightweight and test-oriented;
it performs cosine similarity locally and must be replaced by an encrypted-at-rest production
adapter without changing the application or domain boundaries.
