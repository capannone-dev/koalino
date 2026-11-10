//! Implements the `KeychainStore` port with the operating system's credential store.
//!
//! One entry holds the random 32-byte voice-embedding key as a binary secret:
//!
//! - **macOS:** Keychain Services generic password, service `note-taker`, account
//!   `encryption-key`.
//! - **Windows:** Credential Manager generic credential with target name
//!   `note-taker:encryption-key`.
//! - **Linux/BSD:** Secret Service item (default collection) with attributes
//!   `service=note-taker` and `username=encryption-key`, reached over D-Bus by the pure-Rust zbus
//!   client.
//!
//! The `keyring` crate initializes the platform store once per process. A machine without one
//! (headless Linux with no session bus or no Secret Service daemon, or an unsupported platform)
//! yields `KeychainUnavailable`, which stops unlocking (no key-file fallback). Error messages
//! are built from the error kind and the platform's reason, never from returned secret bytes.

use std::collections::HashMap;

use keyring_core::{Entry, Error as KeyringError};
use zeroize::Zeroizing;

use crate::{domain::errors::CryptoError, ports::crypto::KeychainStore};

/// Service name of the keychain entry on every platform.
pub const KEYCHAIN_SERVICE: &str = "note-taker";
/// Account (user) name of the keychain entry on every platform.
pub const KEYCHAIN_ACCOUNT: &str = "encryption-key";
/// Label shown for the item in Secret Service user interfaces (Seahorse, KWallet). Passed on
/// every platform; only the Linux Secret Service uses it.
const SECRET_SERVICE_LABEL: &str = "Note Taker voice embedding encryption key";

/// The keychain entry for the voice-embedding key.
pub struct PlatformKeychain {
    /// The entry, or the error that prevented building it (no platform store, invalid name).
    entry: Result<Entry, KeyringError>,
    /// Names the store and entry for messages, e.g. `macOS Keychain item service=… account=…`.
    location: String,
}

impl PlatformKeychain {
    /// Opens the production entry (`note-taker` / `encryption-key`).
    ///
    /// Inputs: None.
    /// Outputs: The adapter. A missing platform store is not an error here; it is reported as
    /// `KeychainUnavailable` by every operation.
    /// Side effects: On first use in the process, initializes the platform credential store
    /// (connects to the D-Bus session bus on Linux). Never prompts the user by itself.
    pub fn new() -> Self {
        Self::for_service(KEYCHAIN_SERVICE)
    }

    /// Opens the `encryption-key` entry under another service name.
    ///
    /// Inputs: The service name; tests use a unique throwaway name.
    /// Outputs: The adapter.
    /// Side effects: As for `new`.
    pub(crate) fn for_service(service: &str) -> Self {
        Self::for_account(service, KEYCHAIN_ACCOUNT, SECRET_SERVICE_LABEL)
    }

    /// Opens another entry, such as the Teams bridge secret (#102).
    ///
    /// Inputs: The service and account names, and the label the Linux Secret Service shows.
    /// Outputs: The adapter.
    /// Side effects: As for `new`.
    pub fn for_account(service: &str, account: &str, label: &str) -> Self {
        Self {
            entry: open_platform_entry(service, account, label),
            location: platform_location(service, account),
        }
    }

    /// Wraps an already built entry, such as a `keyring_core` mock credential.
    ///
    /// Inputs: The entry (or the error that stands in for it) and its location string.
    /// Outputs: The adapter.
    /// Side effects: None.
    #[cfg(test)]
    pub(crate) fn from_entry(entry: Result<Entry, KeyringError>, location: &str) -> Self {
        Self {
            entry,
            location: String::from(location),
        }
    }

    /// Deletes the entry; an absent entry counts as deleted.
    ///
    /// Inputs: None.
    /// Outputs: Success, or `KeychainUnavailable` / `Keychain`.
    /// Side effects: Removes the credential from the platform store. The application never calls
    /// this for the production entry; it exists for tests and manual recovery tooling.
    pub fn delete_key(&self) -> Result<(), CryptoError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(self.classify(&error)),
        }
    }

    /// Borrows the entry or converts the build error.
    fn entry(&self) -> Result<&Entry, CryptoError> {
        self.entry.as_ref().map_err(|error| self.classify(error))
    }

    /// Converts a keyring error into the port's error kinds without copying secret bytes.
    ///
    /// Inputs: The keyring error.
    /// Outputs: `KeychainUnavailable` when no working platform store exists, otherwise
    /// `Keychain`. Both name the entry.
    /// Side effects: None.
    fn classify(&self, error: &KeyringError) -> CryptoError {
        let location = &self.location;
        match error {
            KeyringError::NoDefaultStore => CryptoError::KeychainUnavailable {
                reason: format!("{location}: no credential store is available on this machine"),
            },
            KeyringError::PlatformFailure(platform_error) => CryptoError::KeychainUnavailable {
                reason: format!("{location}: {platform_error}"),
            },
            KeyringError::NotSupportedByStore(detail) => CryptoError::KeychainUnavailable {
                reason: format!("{location}: not supported ({detail})"),
            },
            KeyringError::Invalid(parameter, detail) if parameter == "platform" => {
                CryptoError::KeychainUnavailable {
                    reason: format!("{location}: {detail}"),
                }
            }
            KeyringError::NoStorageAccess(platform_error) => CryptoError::Keychain {
                reason: format!("{location}: access denied or store locked ({platform_error})"),
            },
            // The attached bytes are the stored secret; only the platform's reason is kept.
            KeyringError::BadDataFormat(_, platform_error) => CryptoError::Keychain {
                reason: format!("{location}: the stored secret is malformed ({platform_error})"),
            },
            KeyringError::BadEncoding(_) => CryptoError::Keychain {
                reason: format!("{location}: the stored secret has an unexpected encoding"),
            },
            KeyringError::Ambiguous(_) => CryptoError::Keychain {
                reason: format!("{location}: several matching items exist; remove the duplicates"),
            },
            KeyringError::NoEntry => CryptoError::Keychain {
                reason: format!("{location}: the entry does not exist"),
            },
            other => CryptoError::Keychain {
                reason: format!("{location}: {}", describe_without_secrets(other)),
            },
        }
    }
}

impl Default for PlatformKeychain {
    /// Opens the production entry; see `PlatformKeychain::new`.
    fn default() -> Self {
        Self::new()
    }
}

impl KeychainStore for PlatformKeychain {
    /// Reads the binary secret; a missing entry is `None`.
    fn load_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, CryptoError> {
        match self.entry()?.get_secret() {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(self.classify(&error)),
        }
    }

    /// Creates or replaces the binary secret.
    fn store_key(&self, key_bytes: &[u8]) -> Result<(), CryptoError> {
        self.entry()?
            .set_secret(key_bytes)
            .map_err(|error| self.classify(&error))
    }

    /// Names the platform store and entry.
    fn location(&self) -> &str {
        &self.location
    }
}

/// Describes keyring error kinds that carry no secret data.
///
/// Inputs: The error.
/// Outputs: Its message, or a fixed text for kinds that embed stored bytes.
/// Side effects: None.
fn describe_without_secrets(error: &KeyringError) -> String {
    match error {
        KeyringError::BadEncoding(_) | KeyringError::BadDataFormat(..) => {
            String::from("the stored secret is malformed")
        }
        other => other.to_string(),
    }
}

/// Initializes the platform store and builds the entry with platform-specific modifiers.
///
/// Inputs: The service and account names, and the Secret Service label.
/// Outputs: The entry, or the keyring error (`NoDefaultStore`-like failures when no store
/// exists).
/// Side effects: Initializes the process-wide default credential store once.
fn open_platform_entry(service: &str, account: &str, label: &str) -> Result<Entry, KeyringError> {
    if let Err(initialization_error) = keyring::Entry::store_status() {
        // The status is a shared static and not `Clone`; keep its message as a platform failure,
        // which classifies as unavailable.
        return Err(KeyringError::PlatformFailure(Box::from(format!(
            "credential store initialization failed: {initialization_error}"
        ))));
    }
    #[cfg(target_os = "windows")]
    let windows_target = format!("{service}:{account}");
    #[allow(unused_mut)] // Only some platforms add modifiers.
    let mut modifiers: HashMap<&str, &str> = HashMap::new();
    #[cfg(target_os = "windows")]
    modifiers.insert("target", windows_target.as_str());
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    modifiers.insert("label", label);
    // Other stores have no label.
    #[cfg(not(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )))]
    let _ = label;
    Entry::new_with_modifiers(service, account, &modifiers)
}

/// Names the entry the way the platform's own tools show it.
///
/// Inputs: The service and account names.
/// Outputs: A human-readable location.
/// Side effects: None.
pub(crate) fn platform_location(service: &str, account: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("macOS Keychain item service={service} account={account}")
    } else if cfg!(target_os = "windows") {
        format!("Windows Credential Manager target {service}:{account}")
    } else if cfg!(all(
        unix,
        not(any(target_os = "ios", target_os = "android"))
    )) {
        format!("Secret Service item service={service} username={account}")
    } else {
        format!("platform keychain entry {service}/{account}")
    }
}
