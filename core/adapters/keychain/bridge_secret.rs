//! The Teams bridge's shared secret in the operating system keychain (#102).
//!
//! The browser starts the extension's native messaging host with the browser's own environment,
//! so the host cannot be handed `NOTE_TAKER_TEAMS_BRIDGE_SECRET`. Instead `browser-host install`
//! creates a random secret once in the keychain (`note-taker` / `teams-bridge-secret`), and both
//! the host and the daemon read it there whenever the environment variable is not set. The
//! secret is never logged.

use aes_gcm::aead::{OsRng, rand_core::RngCore};
use zeroize::Zeroizing;

use crate::{
    adapters::keychain::platform_keychain::{KEYCHAIN_SERVICE, PlatformKeychain},
    domain::errors::CryptoError,
    ports::crypto::KeychainStore,
};

/// The keychain account holding the secret.
pub const BRIDGE_SECRET_ACCOUNT: &str = "teams-bridge-secret";
/// What the Linux Secret Service shows for the entry.
const BRIDGE_SECRET_LABEL: &str = "Note Taker online-meeting bridge secret";
/// Random bytes in a new secret.
const SECRET_BYTES: usize = 32;

/// Opens the platform keychain entry of the bridge secret.
///
/// Outputs: The entry (an unavailable store is reported when it is used).
/// Side effects: Initializes the platform credential store on first use.
pub fn bridge_secret_keychain() -> PlatformKeychain {
    PlatformKeychain::for_account(KEYCHAIN_SERVICE, BRIDGE_SECRET_ACCOUNT, BRIDGE_SECRET_LABEL)
}

/// Reads the secret.
///
/// Inputs: The keychain entry.
/// Outputs: The secret, `None` when none was created, or an error when the store fails or the
/// stored bytes are not text.
/// Side effects: Queries the credential store.
pub fn load_bridge_secret(
    keychain: &dyn KeychainStore,
) -> Result<Option<Zeroizing<String>>, CryptoError> {
    let Some(stored) = keychain.load_key()? else {
        return Ok(None);
    };
    let text = std::str::from_utf8(&stored).map_err(|_| CryptoError::Keychain {
        reason: format!("{}: the bridge secret is not text", keychain.location()),
    })?;
    Ok(Some(Zeroizing::new(text.to_owned())))
}

/// Returns the secret, creating a random one when none exists.
///
/// Inputs: The keychain entry.
/// Outputs: The secret (64 hexadecimal characters when created), or a store error.
/// Side effects: May write the credential store.
pub fn ensure_bridge_secret(
    keychain: &dyn KeychainStore,
) -> Result<Zeroizing<String>, CryptoError> {
    if let Some(existing) = load_bridge_secret(keychain)? {
        return Ok(existing);
    }
    let mut random_bytes = Zeroizing::new([0_u8; SECRET_BYTES]);
    OsRng.fill_bytes(random_bytes.as_mut());
    let secret = Zeroizing::new(
        random_bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    keychain.store_key(secret.as_bytes())?;
    Ok(secret)
}
