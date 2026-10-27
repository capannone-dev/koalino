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
| `key_manager.rs` | Resolves the key from a passphrase (Argon2id) or a random key file, and verifies it |
| `*_tests.rs` | Round trips, tampering, wrong keys, key sources, and the performance budget |

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

| | Passphrase (`NOTE_TAKER_ENCRYPTION_PASSPHRASE`) | Random key file (no passphrase) |
| --- | --- | --- |
| Key | Argon2id(passphrase, 16-byte salt); 19 MiB, 2 passes, 1 lane by default | 32 random bytes |
| On disk | Salt and parameters in the database (not secret) | `encryption.key`, mode 0600, in a 0700 directory |
| Location | — | `NOTE_TAKER_ENCRYPTION_KEY_DIR`, else `~/.note_taker` |
| Protects against | Stolen disk or database file | A database file that leaks on its own |
| Does not protect against | Readers of the process environment | Anyone who can read the user's files |
| Lost secret | Forgotten passphrase: embeddings unrecoverable | Deleted key file: embeddings unrecoverable |

The key-file mode needs no user interaction, which is why it is the default. It is weaker
because the key sits next to the data. Use a passphrase on shared or portable machines. On
mobile, set `NOTE_TAKER_ENCRYPTION_KEY_DIR` to the app's private directory; storing the key in
the platform keychain is a follow-up.

`KeyManager::unlock` verifies the key-check value on every start. A wrong passphrase
(`WrongKey`), a deleted key file (`KeyFile`; no replacement is generated), or a switch between
passphrase and key-file mode (`KeySourceMismatch`) stops the store from opening before any
embedding is read or migrated. Changing the passphrase or key (re-keying) is not supported yet.
If two processes initialize the same database at once, the second adopts the first one's record.

Argon2id derivation runs once per process and is CPU-bound, so composition roots call it during
start-up, never on a UI thread.
