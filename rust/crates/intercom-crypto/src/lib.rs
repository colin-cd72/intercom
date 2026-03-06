//! Encryption for the intercom system using AES-256-GCM.
//!
//! Provides authenticated encryption for audio data and signaling messages.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use thiserror::Error;

/// Size of AES-256 key in bytes.
pub const KEY_SIZE: usize = 32;

/// Size of GCM nonce in bytes.
pub const NONCE_SIZE: usize = 12;

/// Size of GCM tag in bytes.
pub const TAG_SIZE: usize = 16;

/// Crypto errors.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Encryption failed")]
    EncryptionFailed,

    #[error("Decryption failed: invalid ciphertext or tag")]
    DecryptionFailed,

    #[error("Invalid key size: expected {expected}, got {actual}")]
    InvalidKeySize { expected: usize, actual: usize },

    #[error("Invalid nonce size: expected {expected}, got {actual}")]
    InvalidNonceSize { expected: usize, actual: usize },

    #[error("Ciphertext too short")]
    CiphertextTooShort,
}

/// AES-256-GCM cipher for authenticated encryption.
pub struct Cipher {
    key: Key<Aes256Gcm>,
    cipher: Aes256Gcm,
    nonce_counter: u64,
}

impl Cipher {
    /// Create a new cipher with a random key.
    pub fn new() -> Self {
        let key = Aes256Gcm::generate_key(&mut OsRng);
        let cipher = Aes256Gcm::new(&key);

        Self {
            key,
            cipher,
            nonce_counter: 0,
        }
    }

    /// Create a cipher from an existing key.
    pub fn from_key(key_bytes: &[u8]) -> Result<Self, CryptoError> {
        if key_bytes.len() != KEY_SIZE {
            return Err(CryptoError::InvalidKeySize {
                expected: KEY_SIZE,
                actual: key_bytes.len(),
            });
        }

        let key = Key::<Aes256Gcm>::from_slice(key_bytes).clone();
        let cipher = Aes256Gcm::new(&key);

        Ok(Self {
            key,
            cipher,
            nonce_counter: 0,
        })
    }

    /// Get the encryption key bytes.
    pub fn key_bytes(&self) -> &[u8] {
        self.key.as_slice()
    }

    /// Generate a unique nonce using counter mode.
    fn generate_nonce(&mut self) -> [u8; NONCE_SIZE] {
        let mut nonce = [0u8; NONCE_SIZE];

        // First 4 bytes: random prefix
        OsRng.fill_bytes(&mut nonce[0..4]);

        // Last 8 bytes: counter
        self.nonce_counter = self.nonce_counter.wrapping_add(1);
        nonce[4..12].copy_from_slice(&self.nonce_counter.to_le_bytes());

        nonce
    }

    /// Encrypt data with authenticated encryption.
    ///
    /// Returns: nonce (12 bytes) || ciphertext || tag (16 bytes)
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let nonce_bytes = self.generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);

        Ok(result)
    }

    /// Encrypt data with additional authenticated data (AAD).
    pub fn encrypt_with_aad(
        &mut self,
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        use aes_gcm::aead::Payload;

        let nonce_bytes = self.generate_nonce();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let payload = Payload {
            msg: plaintext,
            aad,
        };

        let ciphertext = self
            .cipher
            .encrypt(nonce, payload)
            .map_err(|_| CryptoError::EncryptionFailed)?;

        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);

        Ok(result)
    }

    /// Decrypt data.
    ///
    /// Input format: nonce (12 bytes) || ciphertext || tag (16 bytes)
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if ciphertext.len() < NONCE_SIZE + TAG_SIZE {
            return Err(CryptoError::CiphertextTooShort);
        }

        let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
        let encrypted = &ciphertext[NONCE_SIZE..];

        self.cipher
            .decrypt(nonce, encrypted)
            .map_err(|_| CryptoError::DecryptionFailed)
    }

    /// Decrypt data with additional authenticated data (AAD).
    pub fn decrypt_with_aad(&self, ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
        use aes_gcm::aead::Payload;

        if ciphertext.len() < NONCE_SIZE + TAG_SIZE {
            return Err(CryptoError::CiphertextTooShort);
        }

        let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
        let encrypted = &ciphertext[NONCE_SIZE..];

        let payload = Payload {
            msg: encrypted,
            aad,
        };

        self.cipher
            .decrypt(nonce, payload)
            .map_err(|_| CryptoError::DecryptionFailed)
    }
}

impl Clone for Cipher {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            cipher: Aes256Gcm::new(&self.key),
            nonce_counter: 0, // Reset counter for clone
        }
    }
}

/// Generate a random encryption key.
pub fn generate_key() -> [u8; KEY_SIZE] {
    let mut key = [0u8; KEY_SIZE];
    OsRng.fill_bytes(&mut key);
    key
}

/// Derive a key from a password using a simple KDF.
/// For production, use a proper KDF like Argon2 or scrypt.
pub fn derive_key_simple(password: &str, salt: &[u8]) -> [u8; KEY_SIZE] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut key = [0u8; KEY_SIZE];

    // Simple PBKDF-like derivation (NOT cryptographically secure for production)
    // Use argon2 or scrypt in production
    for i in 0..KEY_SIZE {
        let mut hasher = DefaultHasher::new();
        password.hash(&mut hasher);
        salt.hash(&mut hasher);
        (i as u64).hash(&mut hasher);
        key[i] = (hasher.finish() & 0xFF) as u8;
    }

    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let mut cipher = Cipher::new();
        let plaintext = b"Hello, World!";

        let ciphertext = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let mut cipher = Cipher::new();
        let plaintext = b"";

        let ciphertext = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_large() {
        let mut cipher = Cipher::new();
        let plaintext: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();

        let ciphertext = cipher.encrypt(&plaintext).unwrap();
        let decrypted = cipher.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_keys() {
        let mut cipher1 = Cipher::new();
        let cipher2 = Cipher::new();

        let plaintext = b"Secret message";
        let ciphertext = cipher1.encrypt(plaintext).unwrap();

        // Different key should fail to decrypt
        let result = cipher2.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext() {
        let mut cipher = Cipher::new();
        let plaintext = b"Hello, World!";

        let mut ciphertext = cipher.encrypt(plaintext).unwrap();

        // Tamper with the ciphertext
        if let Some(byte) = ciphertext.get_mut(NONCE_SIZE + 5) {
            *byte ^= 0xFF;
        }

        let result = cipher.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_decrypt_with_aad() {
        let mut cipher = Cipher::new();
        let plaintext = b"Hello, World!";
        let aad = b"channel:main";

        let ciphertext = cipher.encrypt_with_aad(plaintext, aad).unwrap();
        let decrypted = cipher.decrypt_with_aad(&ciphertext, aad).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_aad() {
        let mut cipher = Cipher::new();
        let plaintext = b"Hello, World!";
        let aad1 = b"channel:main";
        let aad2 = b"channel:other";

        let ciphertext = cipher.encrypt_with_aad(plaintext, aad1).unwrap();

        // Wrong AAD should fail
        let result = cipher.decrypt_with_aad(&ciphertext, aad2);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_key() {
        let key = generate_key();
        let mut cipher1 = Cipher::from_key(&key).unwrap();
        let cipher2 = Cipher::from_key(&key).unwrap();

        let plaintext = b"Test message";
        let ciphertext = cipher1.encrypt(plaintext).unwrap();
        let decrypted = cipher2.decrypt(&ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_invalid_key_size() {
        let result = Cipher::from_key(&[0u8; 16]);
        assert!(matches!(result, Err(CryptoError::InvalidKeySize { .. })));
    }

    #[test]
    fn test_nonce_uniqueness() {
        let mut cipher = Cipher::new();
        let plaintext = b"Same message";

        let ct1 = cipher.encrypt(plaintext).unwrap();
        let ct2 = cipher.encrypt(plaintext).unwrap();

        // Same plaintext should produce different ciphertext (different nonce)
        assert_ne!(ct1, ct2);

        // Both should decrypt correctly
        assert_eq!(cipher.decrypt(&ct1).unwrap(), plaintext);
        assert_eq!(cipher.decrypt(&ct2).unwrap(), plaintext);
    }
}
