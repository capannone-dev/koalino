//! Stores the random voice-embedding key in the operating system's credential store (Issue #40).

/// Implements `KeychainStore` with the `keyring` crate.
pub mod platform_keychain;
#[cfg(test)]
mod platform_keychain_tests;

/// Implements `KeychainStore` over a key the host app unwrapped (the Android Keystore).
pub mod host_provided_keychain;
#[cfg(test)]
mod host_provided_keychain_tests;
