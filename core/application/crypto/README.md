<!-- This directory holds the at-rest encryption service for voice embeddings and its key management. -->

# Voice embedding encryption

Voice embeddings are biometric PII. Every embedding the SQLite adapters store (identity voice
variants, unknown-speaker cluster representatives and members, and review-clip embeddings) is
sealed here before it reaches disk. The adapters only see the `EmbeddingCipher` port
(`core/ports/crypto.rs`); they never handle keys or cipher primitives.

## Files

| File | Purpose |
| --- | --- |
| `voice_encryptor.rs` | AES-256-GCM sealing (`encrypt_embedding`, `decrypt_embedding`, `VoiceEncryptor`) |
| `key_manager.rs` | Resolves the key from a passphrase (Argon2id), the platform keychain, or a random key file; migrates the key file into the keychain; verifies the key |
| `*_tests.rs` | Round trips, tampering, wrong keys, key sources, keychain migration and fallback (with a mock keychain), and the performance budget |

## Encryption scheme

```text
sealed value = "NTE1" (4 B) ‖ random nonce (12 B) ‖ AES-256-GCM ciphertext ‖ tag (16 B)
plaintext    = embedding as little-endian f32 bytes
AAD          = "NTE1" ‖ row context, e.g. "voice_variants:<variant uuid>"
```

- A fresh 96-bit nonce comes from the operating system for every seal.
- The tag authenticates the header, nonce, ciphertext, and row context. A modified byte, a wrong
  key, or a value copied onto another row fails with `AuthenticationFailed`. A missing header or
  a truncated value is `MalformedCiphertext`. Neither case ever falls back to plaintext.
- `VoiceEncryptor` prepares the key schedule once. Sealing and opening a 256-dimension embedding
  takes microseconds; the benchmark test requires 1000 round trips in under one second in the
  unoptimized test build.
- Keys (`EncryptionKey`), passphrases, and intermediate plaintext buffers are wiped on drop
  (`zeroize`) and redacted from `Debug` output. Nothing in this module logs.

## Key management and its security tradeoff

The key source is chosen when a database is first encrypted and recorded in its
`encryption_meta` row, together with a key-check value (a known plaintext sealed with the key).
There are two sources. A **passphrase**, or a **random key** that lives in the platform keychain
(Issue #40) or, as a fallback, in a key file. The stored value for the random key is still
`key_file`, so existing databases keep working; error messages call it "random key".

| | Passphrase (`NOTE_TAKER_ENCRYPTION_PASSPHRASE`) | Random key in the platform keychain (default) | Random key in a key file (fallback) |
| --- | --- | --- | --- |
| Key | Argon2id(passphrase, 16-byte salt); 19 MiB, 2 passes, 1 lane by default | 32 random bytes | 32 random bytes |
| Stored | Salt and parameters in the database (not secret) | macOS Keychain, Windows Credential Manager, or Secret Service | `encryption.key`, mode 0600, in a 0700 directory |
| Location | — | Service `note-taker`, account `encryption-key` (see below) | `NOTE_TAKER_ENCRYPTION_KEY_DIR`, else `~/.note_taker` |
| Protects against | Stolen disk or database file | Leaked database files and offline disk copies; the OS can lock the entry behind the login | A database file that leaks on its own |
| Does not protect against | Readers of the process environment | Malware running as the logged-in user | Anyone who can read the user's files |
| Lost secret | Forgotten passphrase: embeddings unrecoverable | Deleted entry with no key file: embeddings unrecoverable | Deleted key file: embeddings unrecoverable |

### Configuration

| Variable | Default | Effect |
| --- | --- | --- |
| `NOTE_TAKER_ENCRYPTION_PASSPHRASE` | unset | Derive the key from the passphrase. The keychain and key file are never used, and the passphrase is never stored. |
| `NOTE_TAKER_KEYCHAIN_ENABLED` | `true` | `false`, `0`, `no`, or `off` (any case) forces the key file. Any other value keeps the keychain. |
| `NOTE_TAKER_ENCRYPTION_KEY_DIR` | `~/.note_taker` | Directory of the key file (keychain disabled) and of an old key file to migrate. |

Keychain entries (implemented in `core/adapters/keychain/`):

| Platform | Store | Entry |
| --- | --- | --- |
| macOS | Keychain Services | Generic password, service `note-taker`, account `encryption-key` |
| Windows | Credential Manager | Generic credential, target name `note-taker:encryption-key` |
| Linux / BSD | Secret Service (D-Bus) | Attributes `service=note-taker`, `username=encryption-key` |
| Android | Android Keystore | The Flutter app unwraps the key with a Keystore key (StrongBox when present) and hands it to the core through `note_taker_provide_encryption_key`; see `ui/android/README.md` |
| iOS | none yet | Set `NOTE_TAKER_KEYCHAIN_ENABLED=false` and `NOTE_TAKER_ENCRYPTION_KEY_DIR` to the app's private directory |

The entry is per user, not per database: every database on the machine that uses a random key
shares it, just as they shared `~/.note_taker/encryption.key`.

### Resolution and migration

Without a passphrase, `KeyManager` resolves the random key on every unlock:

```text
keychain disabled or not injected ─▶ key file (create for a new database)
keychain entry present ───────────▶ use it
   └ key file also present: same key → delete the file (interrupted migration)
                            different → keep the file and try both against the key check
keychain entry absent
   ├ key file present ─▶ migrate: store in keychain, read back, compare
   │                       equal → delete the file      otherwise → keep the file, Keychain error
   ├ new database ──────▶ generate into the keychain (read back); if that fails → Keychain error
   └ encrypted database ▶ KeyMissing (names the keychain entry and the key file path)
keychain unavailable or failing ─▶ KeychainUnavailable (no store on this machine, e.g. headless
                                   Linux) or Keychain (locked, cancelled, or access denied)
```

- **Migration** happens automatically on the first start with the keychain enabled. The key
  file is deleted only after the keychain returns exactly the same 32 bytes. On any failure the
  file stays and unlocking fails, so the next start, with a working keychain, retries it. The
  deleted file is unlinked, not overwritten. On SSDs and copy-on-write file systems, overwriting
  would not reliably erase the old blocks anyway.
- **No fallback** (owner decision, `DECISION_LOG.md`): when the keychain is enabled but locked,
  cancelled, denied, or missing, unlocking fails. The daemon and the CLI exit with the error; the
  Flutter app shows it with Retry. Headless servers without a keychain set a passphrase, or
  disable the keychain explicitly to use a key file.
- **Conflict**: a key file left from an older version can differ from the keychain entry. Both
  are kept. The key file is never deleted while it
  differs from the keychain entry, and unlocking tries both.
- **Corrupt entry**: an entry that is not 32 bytes is a `Keychain` error. It is never replaced
  automatically, because it may belong to existing databases.
- **Missing key**: an encrypted database whose key is in neither place fails with `KeyMissing`.
  No replacement key is generated, since a new key could not decrypt the existing embeddings.

To move a key back out of the keychain (for example to copy it to another machine), read the
32-byte secret with the platform tool, write it to `encryption.key` (mode 0600), and set
`NOTE_TAKER_KEYCHAIN_ENABLED=false`.

`KeyManager::unlock` verifies the key-check value on every start. A wrong passphrase
(`WrongKey`), a deleted key file with the keychain disabled (`KeyFile`), a key missing from both
the keychain and the file (`KeyMissing`), or a switch between passphrase and random-key mode
(`KeySourceMismatch`) stops the store from opening before any embedding is read or migrated.
Changing the passphrase or key (re-keying) is not supported yet. If two processes initialize the
same database at once, the second adopts the first one's record.

Argon2id derivation and keychain access run once per process, during start-up. Derivation is
CPU-bound and the keychain may show an unlock prompt, so composition roots never run them on a
UI thread.
