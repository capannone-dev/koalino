//! Stores the random voice-embedding key in the operating system's credential store (Issue #40).

/// Implements `KeychainStore` with the `keyring` crate.
pub mod platform_keychain;
#[cfg(test)]
mod platform_keychain_tests;
