//! Verifies the checked-in generated code in `generated/` matches `teams_bridge.proto`.
//!
//! To regenerate after editing the schema, run the protocol tests with
//! `NOTE_TAKER_REGENERATE_PROTOCOL=1` (for example
//! `NOTE_TAKER_REGENERATE_PROTOCOL=1 cargo test --lib protocol::generated_code_tests`); the test
//! then overwrites `generated/` instead of comparing.

use std::path::{Path, PathBuf};

/// Directory holding the schema.
fn protocol_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("core")
        .join("protocol")
}

/// Runs rust-protobuf's pure-Rust code generator into `output_directory`.
fn generate_into(output_directory: &Path) {
    let protocol_directory = protocol_directory();
    protobuf_codegen::Codegen::new()
        .pure()
        .include(&protocol_directory)
        .input(protocol_directory.join("teams_bridge.proto"))
        .out_dir(output_directory)
        .run()
        .unwrap_or_else(|error| panic!("code generation should succeed: {error:#}"));
}

/// Reads a generated file, panicking with its path when missing.
fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", path.display()))
}

/// Verifies (or, when requested, rewrites) the checked-in generated code.
#[test]
fn checked_in_generated_code_matches_the_schema() {
    let checked_in_directory = protocol_directory().join("generated");
    if std::env::var_os("NOTE_TAKER_REGENERATE_PROTOCOL").is_some() {
        generate_into(&checked_in_directory);
        return;
    }
    let fresh_directory =
        std::env::temp_dir().join(format!("note-taker-protocol-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&fresh_directory)
        .unwrap_or_else(|error| panic!("temp directory should be created: {error}"));
    generate_into(&fresh_directory);
    let mismatched_files: Vec<&str> = ["mod.rs", "teams_bridge.rs"]
        .into_iter()
        .filter(|file_name| {
            read(&fresh_directory.join(file_name)) != read(&checked_in_directory.join(file_name))
        })
        .collect();
    let _ = std::fs::remove_dir_all(&fresh_directory);
    assert!(
        mismatched_files.is_empty(),
        "core/protocol/generated/{mismatched_files:?} is stale; rerun with \
         NOTE_TAKER_REGENERATE_PROTOCOL=1"
    );
}
