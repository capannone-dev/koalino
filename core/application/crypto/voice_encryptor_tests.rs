//! Specifies AES-256-GCM sealing of voice embeddings: round trips, nonce freshness, tamper and
//! wrong-key rejection, row binding, and the real-time performance budget.

use std::time::{Duration, Instant};

use super::voice_encryptor::{
    EncryptionKey, SEALED_EMBEDDING_HEADER, VoiceEncryptor, decrypt_embedding,
    decrypt_embedding_with_context, encrypt_embedding, encrypt_embedding_with_context,
};
use crate::{domain::errors::CryptoError, ports::crypto::EmbeddingCipher};

/// Size of the format header, nonce, and authentication tag added to every embedding.
const SEALING_OVERHEAD_BYTES: usize = 4 + 12 + 16;

/// Returns a fixed test key; production keys come from `KeyManager`.
fn test_key(fill_byte: u8) -> [u8; 32] {
    [fill_byte; 32]
}

/// Returns a deterministic 256-dimension embedding resembling a speaker-model output.
fn sample_embedding(seed: usize) -> Vec<f32> {
    (0..256)
        .map(|dimension| ((dimension * 31 + seed * 7) % 97) as f32 / 97.0 - 0.5)
        .collect()
}

/// Encrypt embedding → decrypt returns the original.
#[test]
fn decrypting_an_encrypted_embedding_returns_the_original() {
    let embedding = sample_embedding(1);

    let sealed = encrypt_embedding(&embedding, &test_key(7))
        .unwrap_or_else(|error| panic!("embedding must encrypt: {error}"));

    assert_eq!(decrypt_embedding(&sealed, &test_key(7)), Ok(embedding));
}

/// Every encryption uses a fresh nonce, the documented layout, and hides the plaintext.
#[test]
fn each_encryption_uses_a_fresh_nonce_and_hides_the_plaintext() {
    let embedding = sample_embedding(2);
    let plaintext_bytes: Vec<u8> = embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();

    let first = encrypt_embedding(&embedding, &test_key(7))
        .unwrap_or_else(|error| panic!("embedding must encrypt: {error}"));
    let second = encrypt_embedding(&embedding, &test_key(7))
        .unwrap_or_else(|error| panic!("embedding must encrypt: {error}"));

    assert_ne!(
        first, second,
        "a repeated nonce would leak equal embeddings"
    );
    assert_ne!(first[4..16], second[4..16]);
    assert_eq!(&first[..4], SEALED_EMBEDDING_HEADER);
    assert_eq!(first.len(), plaintext_bytes.len() + SEALING_OVERHEAD_BYTES);
    assert!(
        !first.windows(16).any(|window| plaintext_bytes
            .chunks(16)
            .any(|plaintext_chunk| plaintext_chunk == window)),
        "no 16-byte run of the plaintext may appear in the sealed value"
    );
}

/// Encrypt with one key → decrypting with another key fails.
#[test]
fn decrypting_with_the_wrong_key_fails() {
    let sealed = encrypt_embedding(&sample_embedding(3), &test_key(7))
        .unwrap_or_else(|error| panic!("embedding must encrypt: {error}"));

    assert_eq!(
        decrypt_embedding(&sealed, &test_key(8)),
        Err(CryptoError::AuthenticationFailed)
    );
}

/// Flipping any bit of the nonce, ciphertext, or tag is detected.
#[test]
fn any_modified_byte_is_rejected() {
    let sealed = encrypt_embedding(&[0.25, -0.5, 1.0], &test_key(7))
        .unwrap_or_else(|error| panic!("embedding must encrypt: {error}"));

    for byte_index in 0..sealed.len() {
        let mut tampered = sealed.clone();
        tampered[byte_index] ^= 0x01;
        assert!(
            decrypt_embedding(&tampered, &test_key(7)).is_err(),
            "modified byte {byte_index} must be rejected"
        );
    }
}

/// A sealed value only opens for the row context it was sealed for.
#[test]
fn sealed_values_are_bound_to_their_row_context() {
    let sealed = encrypt_embedding_with_context(&sample_embedding(4), &test_key(7), b"row-a")
        .unwrap_or_else(|error| panic!("embedding must encrypt: {error}"));

    assert_eq!(
        decrypt_embedding_with_context(&sealed, &test_key(7), b"row-a"),
        Ok(sample_embedding(4))
    );
    assert_eq!(
        decrypt_embedding_with_context(&sealed, &test_key(7), b"row-b"),
        Err(CryptoError::AuthenticationFailed)
    );
}

/// Truncated values and values without the format header are malformed, not plaintext.
#[test]
fn malformed_values_are_rejected() {
    let sealed = encrypt_embedding(&[1.0, 2.0], &test_key(7))
        .unwrap_or_else(|error| panic!("embedding must encrypt: {error}"));
    let mut wrong_header = sealed.clone();
    wrong_header[..4].copy_from_slice(b"XXXX");
    let plaintext: Vec<u8> = [1.0_f32, 2.0]
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();

    for malformed in [Vec::new(), sealed[..20].to_vec(), wrong_header, plaintext] {
        assert_eq!(
            decrypt_embedding(&malformed, &test_key(7)),
            Err(CryptoError::MalformedCiphertext)
        );
    }
}

/// Empty embeddings (clips stored before an embedding was derived) still round-trip.
#[test]
fn empty_embeddings_round_trip() {
    let sealed = encrypt_embedding(&[], &test_key(7))
        .unwrap_or_else(|error| panic!("empty embedding must encrypt: {error}"));

    assert_eq!(sealed.len(), SEALING_OVERHEAD_BYTES);
    assert_eq!(decrypt_embedding(&sealed, &test_key(7)), Ok(Vec::new()));
}

/// The port implementation seals with its key and verifies its own key-check value.
#[test]
fn voice_encryptor_implements_the_cipher_port_and_key_check() {
    let encryptor = VoiceEncryptor::new(&EncryptionKey::from_bytes(test_key(7)));
    let other_encryptor = VoiceEncryptor::new(&EncryptionKey::from_bytes(test_key(8)));

    let sealed = encryptor
        .seal_embedding("voice_variants:1", &sample_embedding(5))
        .unwrap_or_else(|error| panic!("embedding must seal: {error}"));
    let key_check = encryptor
        .seal_key_check()
        .unwrap_or_else(|error| panic!("key check must seal: {error}"));

    assert_eq!(
        encryptor.open_embedding("voice_variants:1", &sealed),
        Ok(sample_embedding(5))
    );
    assert!(
        other_encryptor
            .open_embedding("voice_variants:1", &sealed)
            .is_err()
    );
    assert_eq!(encryptor.verify_key_check(&key_check), Ok(()));
    assert_eq!(
        other_encryptor.verify_key_check(&key_check),
        Err(CryptoError::WrongKey)
    );
}

/// Two ephemeral encryptors never share a key.
#[test]
fn ephemeral_encryptors_use_independent_random_keys() {
    let sealed = VoiceEncryptor::ephemeral()
        .seal_embedding("context", &[1.0])
        .unwrap_or_else(|error| panic!("embedding must seal: {error}"));

    assert!(
        VoiceEncryptor::ephemeral()
            .open_embedding("context", &sealed)
            .is_err()
    );
}

/// Key material never appears in debug output.
#[test]
fn keys_are_redacted_in_debug_output() {
    let debug_output = format!("{:?}", EncryptionKey::from_bytes([0xAB; 32]));

    assert!(!debug_output.to_lowercase().contains("ab, "));
    assert!(!debug_output.contains("171"));
    assert!(debug_output.contains("redacted"));
}

/// Benchmark: sealing and opening 1000 embeddings takes under one second in total (< 1 ms each).
///
/// The AES-GCM crates build optimized in every profile (see `Cargo.toml`), so this measures real
/// cipher cost. The best of three batches is compared, so one scheduling stall on a shared CI
/// runner running the other tests in parallel cannot fail the budget.
#[test]
fn sealing_and_opening_one_thousand_embeddings_takes_under_one_second() {
    let encryptor = VoiceEncryptor::new(&EncryptionKey::from_bytes(test_key(7)));
    let embeddings: Vec<Vec<f32>> = (0..1_000).map(sample_embedding).collect();

    let mut fastest_batch = Duration::MAX;
    for _ in 0..3 {
        let started_at = Instant::now();
        for (row_index, embedding) in embeddings.iter().enumerate() {
            let context = format!("voice_variants:{row_index}");
            let sealed = encryptor
                .seal_embedding(&context, embedding)
                .unwrap_or_else(|error| panic!("embedding must seal: {error}"));
            let opened = encryptor
                .open_embedding(&context, &sealed)
                .unwrap_or_else(|error| panic!("embedding must open: {error}"));
            assert_eq!(opened.len(), embedding.len());
        }
        fastest_batch = fastest_batch.min(started_at.elapsed());
    }
    let elapsed = fastest_batch;

    assert!(
        elapsed < Duration::from_secs(1),
        "1000 seal/open round trips took {elapsed:?}"
    );
}

/// Arbitrary secrets (such as the P2P node key) round-trip and never appear in the sealed bytes.
#[test]
fn sealing_a_secret_round_trips_and_hides_the_plaintext() {
    let encryptor = VoiceEncryptor::new(&EncryptionKey::from_bytes(test_key(7)));
    let secret = b"ed25519-node-secret-material-32b".to_vec();

    let sealed = encryptor
        .seal_secret("p2p_node_identity", &secret)
        .unwrap_or_else(|error| panic!("secret must seal: {error}"));
    let opened = encryptor
        .open_secret("p2p_node_identity", &sealed)
        .unwrap_or_else(|error| panic!("secret must open: {error}"));

    assert_eq!(opened.as_slice(), secret.as_slice());
    assert!(!sealed.windows(secret.len()).any(|window| window == secret));
}

/// A secret sealed for one context cannot be opened as another, nor with another key.
#[test]
fn opening_a_secret_requires_its_context_and_key() {
    let encryptor = VoiceEncryptor::new(&EncryptionKey::from_bytes(test_key(7)));
    let other_encryptor = VoiceEncryptor::new(&EncryptionKey::from_bytes(test_key(8)));
    let sealed = encryptor
        .seal_secret("p2p_node_identity", b"secret")
        .unwrap_or_else(|error| panic!("secret must seal: {error}"));

    assert_eq!(
        encryptor.open_secret("voice_variants:x", &sealed).err(),
        Some(CryptoError::AuthenticationFailed)
    );
    assert_eq!(
        other_encryptor
            .open_secret("p2p_node_identity", &sealed)
            .err(),
        Some(CryptoError::AuthenticationFailed)
    );
}
