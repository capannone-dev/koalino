//! Stores the random voice-embedding key in the operating system's credential store (Issue #40),
//! and the Teams bridge's shared secret (Issue #102).

/// Reads and creates the Teams bridge secret shared by the daemon and the browser host.
pub mod bridge_secret;
#[cfg(test)]
mod bridge_secret_tests;

/// Implements `KeychainStore` with the `keyring` crate.
pub mod platform_keychain;
#[cfg(test)]
mod platform_keychain_tests;

/// Implements `KeychainStore` over a key the host app unwrapped (the Android Keystore).
pub mod host_provided_keychain;
#[cfg(test)]
mod host_provided_keychain_tests;

/// Implements `KeychainStore` as a file sealed under a host-protected key (Android).
pub mod sealed_file_keychain;
#[cfg(test)]
mod sealed_file_keychain_tests;
