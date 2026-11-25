// Keeps the database encryption key wrapped by a non-exportable Android Keystore key, so the
// raw key never rests on disk (owner decision: no key file; see DECISION_LOG.md).
package com.example.note_taker_ui

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Loads, migrates, or creates the 32-byte database key in `keyDirectory`.
 *
 * Files: `encryption.key.wrapped` holds [FORMAT_VERSION][12-byte IV][AES-GCM ciphertext + tag]
 * of the key, sealed by the Keystore key [WRAPPING_KEY_ALIAS]. An old plain `encryption.key`
 * from earlier builds is wrapped, verified, and deleted.
 */
class EncryptionKeyVault(private val keyDirectory: File) {
    private val wrappedFile = File(keyDirectory, "encryption.key.wrapped")
    private val plainFile = File(keyDirectory, "encryption.key")

    /**
     * Returns the database key; the caller zero-fills it after use.
     *
     * Outputs: 32 bytes. Throws when the Keystore or the wrapped file is unusable; a key is
     * never regenerated over an existing wrapped file, because a new key could not open the
     * existing database.
     * Side effects: May create the Keystore key, write the wrapped file, and delete the old
     * plain key file once the wrapped copy is verified.
     */
    fun loadOrCreateKey(): ByteArray {
        if (wrappedFile.exists()) {
            val key = unwrap(wrappedFile.readBytes())
            removeMigratedPlainFile(key)
            return key
        }
        val key = if (plainFile.exists()) {
            plainFile.readBytes().also {
                require(it.size == KEY_LENGTH) { "the old key file is not $KEY_LENGTH bytes" }
            }
        } else {
            ByteArray(KEY_LENGTH).also { SecureRandom().nextBytes(it) }
        }
        storeVerified(key)
        removeMigratedPlainFile(key)
        return key
    }

    /**
     * Keeps a restored backup's key (#167) wrapped by this app's Keystore key.
     *
     * Inputs: The 32-byte key from the backup; the caller zero-fills it afterwards.
     * Outputs: None. Throws when the folder already holds a wrapped key (a restore writes into a
     * fresh folder) or the key cannot be wrapped and verified.
     * Side effects: Creates the folder and writes `encryption.key.wrapped`.
     */
    fun storeRestoredKey(key: ByteArray) {
        require(key.size == KEY_LENGTH) { "the restored key is not $KEY_LENGTH bytes" }
        check(!wrappedFile.exists()) { "the restore folder already holds a key" }
        storeVerified(key)
    }

    /**
     * Keeps the automatic backups' passphrase (#170) wrapped by the same Keystore key, so the
     * backup job can run while nobody is there to type it. The `keys` folder is never backed up.
     *
     * Inputs: The UTF-8 passphrase; the caller wipes it. Side effects: Writes
     * `backup-passphrase.wrapped` atomically.
     */
    fun storeBackupPassphrase(passphrase: ByteArray) {
        require(passphrase.isNotEmpty()) { "the passphrase is empty" }
        keyDirectory.mkdirs()
        val temporary = File(keyDirectory, "$BACKUP_PASSPHRASE_FILE.tmp")
        temporary.writeBytes(wrap(passphrase))
        check(temporary.renameTo(File(keyDirectory, BACKUP_PASSPHRASE_FILE))) {
            "the wrapped passphrase could not be saved"
        }
    }

    /** The stored backup passphrase, or null when none is kept; the caller wipes it. */
    fun loadBackupPassphrase(): ByteArray? {
        val file = File(keyDirectory, BACKUP_PASSPHRASE_FILE)
        return if (file.exists()) unwrapSecret(file.readBytes()) else null
    }

    /** Deletes the stored backup passphrase. */
    fun forgetBackupPassphrase() {
        File(keyDirectory, BACKUP_PASSPHRASE_FILE).delete()
    }

    /** Writes the wrapped key atomically and checks that it unwraps to the same bytes. */
    private fun storeVerified(key: ByteArray) {
        keyDirectory.mkdirs()
        val temporary = File(keyDirectory, "encryption.key.wrapped.tmp")
        temporary.writeBytes(wrap(key))
        check(temporary.renameTo(wrappedFile)) { "the wrapped key could not be saved" }
        val readBack = unwrap(wrappedFile.readBytes())
        val matches = readBack.contentEquals(key)
        readBack.fill(0)
        check(matches) { "the wrapped key did not unwrap to the same key" }
    }

    /** Deletes the old plain key file when it holds `key`; keeps a differing one untouched. */
    private fun removeMigratedPlainFile(key: ByteArray) {
        if (!plainFile.exists()) return
        val plain = plainFile.readBytes()
        if (plain.contentEquals(key)) plainFile.delete()
        plain.fill(0)
    }

    private fun wrap(key: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, wrappingKey())
        val iv = cipher.iv
        check(iv.size == IV_LENGTH) { "unexpected IV length" }
        return byteArrayOf(FORMAT_VERSION) + iv + cipher.doFinal(key)
    }

    private fun unwrap(wrapped: ByteArray): ByteArray {
        val key = unwrapSecret(wrapped)
        if (key.size != KEY_LENGTH) {
            key.fill(0)
            error("the unwrapped key is not $KEY_LENGTH bytes")
        }
        return key
    }

    /** Opens anything `wrap` sealed; the caller wipes the result. */
    private fun unwrapSecret(wrapped: ByteArray): ByteArray {
        require(wrapped.size > 1 + IV_LENGTH && wrapped[0] == FORMAT_VERSION) {
            "the wrapped key file has an unknown format"
        }
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(
            Cipher.DECRYPT_MODE,
            existingWrappingKey() ?: error("the Keystore key that wraps the database key is gone"),
            GCMParameterSpec(TAG_BITS, wrapped, 1, IV_LENGTH),
        )
        return cipher.doFinal(wrapped, 1 + IV_LENGTH, wrapped.size - 1 - IV_LENGTH)
    }

    private fun existingWrappingKey(): SecretKey? {
        val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        return keyStore.getKey(WRAPPING_KEY_ALIAS, null) as SecretKey?
    }

    /** Returns the Keystore key, creating it in StrongBox when available, else the TEE. */
    private fun wrappingKey(): SecretKey =
        existingWrappingKey() ?: try {
            generateWrappingKey(strongBox = true)
        } catch (_: StrongBoxUnavailableException) {
            generateWrappingKey(strongBox = false)
        }

    private fun generateWrappingKey(strongBox: Boolean): SecretKey {
        val spec = KeyGenParameterSpec.Builder(
            WRAPPING_KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
        )
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(256)
            .setIsStrongBoxBacked(strongBox)
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
            .apply { init(spec) }
            .generateKey()
    }

    private companion object {
        const val BACKUP_PASSPHRASE_FILE = "backup-passphrase.wrapped"
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val WRAPPING_KEY_ALIAS = "note_taker_database_key_wrapper"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val FORMAT_VERSION: Byte = 1
        const val KEY_LENGTH = 32
        const val IV_LENGTH = 12
        const val TAG_BITS = 128
    }
}
