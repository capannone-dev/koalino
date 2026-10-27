<!-- Provides the project entry point and links to its architecture and operating contracts. -->

# Note Taker

Note Taker is a decentralized, offline-first meeting note application. Its Rust core uses hexagonal architecture so AI, encrypted P2P, CRDT, and storage implementations remain replaceable.

## Development

The initial Rust scaffold is verified with `make rust-lint`, `make rust-test`, and `make build-server`. The Flutter targets are intentionally present in the Makefile for the future UI workspace.

## Project documents

- [Architecture](ARCHITECTURE.md)
- [Requirements](REQUIREMENTS.md)
- [API contract](API_SPECS.md)
- [Engineering context](CONTEXT.md)
- [Task board](TASK_BOARD.md)
- [Decision log](DECISION_LOG.md)
