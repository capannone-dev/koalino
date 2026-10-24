<!-- Tracks the implementation status of work packages and must be consulted before future work begins. -->

# Task board

## WP1: Core Domain & Ports

| Issue | Status | Scope |
| --- | --- | --- |
| #1 Initialize Rust Core scaffolding | Ready for review | Hexagonal crate layout, typed domain models, ports, application recording use case, tracing observability, Makefile, and CI. |

## Follow-up work

- Define validation and IDs suitable for CRDT replication before storage adapters are introduced.
- Add mock port implementations and expanded error-path tests under WP6.
- Implement libp2p, SQLite/Automerge, and AI adapters only in their assigned work packages.
