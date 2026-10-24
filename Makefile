# Project verification entry points for the Note Taker Rust and Flutter components.

.PHONY: rust-lint rust-test flutter-lint flutter-test build-desktop build-server

rust-lint:
	cargo fmt --check
	cargo clippy --all-targets --all-features -- -D warnings

rust-test:
	cargo test --lib

flutter-lint:
	flutter format --set-exit-if-changed .
	flutter analyze

flutter-test:
	flutter test

build-desktop:
	flutter build linux

build-server:
	cargo build --bin daemon
