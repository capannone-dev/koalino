<!-- Tracks the implementation status of work packages and must be consulted before future work begins. -->

# Task board

## WP1: Core Domain & Ports

| Issue | Status | Scope |
| --- | --- | --- |
| #1 Initialize Rust Core scaffolding | Ready for review | Hexagonal crate layout, typed domain models, ports, application recording use case, tracing observability, Makefile, and CI. |

## WP2: Storage & Sync Adapters

| Issue | Status | Scope |
| --- | --- | --- |
| #2 SQLite NoteRepository and Automerge CRDT integration | Ready for review | In-memory TDD coverage, SQLite meeting schema, opaque CRDT BLOB persistence, corruption handling, and offline merge service. |

## WP3: Audio Capture

| Issue | Status | Scope |
| --- | --- | --- |
| #3 AudioCapture port and local recording state machine | Ready for review | Portable PCM chunk/state subscriptions, TDD state-transition coverage, local deterministic adapter, and structured transition logging. |

## WP4: AI & Compute Routing

| Issue | Status | Scope |
| --- | --- | --- |
| #4 TranscriptionEngine trait, device manifests, and compute dispatcher | Ready for review | Trait-only transcription dispatch, non-sensitive capability manifests, deterministic local/specific/automatic routing, and mock-only TDD coverage. |

## Follow-up work

- Define validation and IDs suitable for CRDT replication before storage adapters are introduced.
- Add mock port implementations and expanded error-path tests under WP6.
- Implement libp2p and AI adapters only in their assigned work packages.
