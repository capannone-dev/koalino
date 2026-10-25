# Project verification entry points for the Note Taker Rust and Flutter components.

.PHONY: rust-lint rust-test flutter-lint flutter-test build-desktop build-server

rust-lint:
	cargo fmt --check
	cargo clippy --all-targets --all-features -- -D warnings

rust-test:
	cargo test --lib

flutter-lint:
	cd ui && dart format --set-exit-if-changed .
	cd ui && flutter analyze

flutter-test:
	cd ui && flutter test

build-desktop:
	cd ui && flutter build linux

build-server:
	cargo build --bin daemon
