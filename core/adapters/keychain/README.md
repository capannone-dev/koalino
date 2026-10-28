<!-- This directory holds the platform keychain adapter that stores the random voice-embedding key (Issue #40). -->

# Platform keychain adapter

`PlatformKeychain` implements the `KeychainStore` port (`core/ports/crypto.rs`) with the
[`keyring`](https://crates.io/crates/keyring) crate (v4, `v1` mode), which picks the operating
system's credential store. The application layer (`application/crypto/key_manager.rs`) decides
when to read, write, migrate, or fall back; this adapter only reads and writes one entry.

## Files

| File | Purpose |
| --- | --- |
| `platform_keychain.rs` | `PlatformKeychain`, entry names, platform modifiers, error classification |
| `platform_keychain_tests.rs` | Mock-credential tests, plus two `#[ignore]`d tests against the real OS keychain |

## Entry per platform

The secret is the raw 32-byte key (binary, not text).

| Platform | Store | Entry |
| --- | --- | --- |
| macOS | Keychain Services (login keychain) | Generic password, service `note-taker`, account `encryption-key` |
| Windows | Credential Manager | Generic credential, target name `note-taker:encryption-key` |
| Linux / BSD | Secret Service (GNOME Keyring, KWallet, KeePassXC) over D-Bus | Default collection, attributes `service=note-taker`, `username=encryption-key`, label "Note Taker voice embedding encryption key" |
| iOS, Android, others | none (`keyring` v1 mode has no store there) | Always `KeychainUnavailable`, so the key file is used |

Inspecting the entry:

- macOS: Keychain Access → search `note-taker`, or `security find-generic-password -s note-taker -a encryption-key`.
- Windows: Control Panel → Credential Manager → Windows Credentials → `note-taker:encryption-key`.
- Linux: `secret-tool search service note-taker username encryption-key` (the secret is binary).

## Error classification

| `keyring` error | Port error | Key manager reaction |
| --- | --- | --- |
| `NoEntry` on read | `Ok(None)` | Migrate the key file, or create a key (new database), or `KeyMissing` |
| Store initialization failed, `NoDefaultStore`, `PlatformFailure`, `NotSupportedByStore` | `KeychainUnavailable` | Warning, key file fallback |
| `NoStorageAccess` (locked, denied), malformed or ambiguous items, others | `Keychain` | Error log, key file fallback |

Messages name the entry (`location()`) and the platform's reason. Error kinds that carry the
stored bytes (`BadEncoding`, `BadDataFormat`) are reduced to a fixed text, so a secret never
reaches a log or an error message.

## Notes

- On Linux the zbus client is pure Rust, so building needs no `libdbus` headers. A machine
  without a session bus or Secret Service daemon (servers, containers, CI) reports the keychain
  as unavailable.
- Keychain calls block briefly and may show a platform unlock prompt; they run during store
  start-up, off UI threads, like the rest of `open_encrypted_stores`.
- `make rust-test` never touches the real keychain: unit tests use `keyring_core`'s mock
  credential, and composition tests disable the keychain. Run the real-store checks explicitly:
  `cargo test --lib real_platform_keychain -- --ignored`. They use a throwaway
  `note-taker-test-<uuid>` service and delete it.
