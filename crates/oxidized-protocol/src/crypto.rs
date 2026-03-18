//! Cryptographic primitives for the Minecraft protocol.
//!
//! Provides AES-128-CFB8 stream encryption (used after the login handshake),
//! RSA-1024 key pair generation (for the server's identity during login),
//! and RSA decryption (to unwrap the client's shared secret).
//!
//! See [ADR-009](../../docs/adr/adr-009-encryption-compression.md) for design rationale.

use aes::Aes128;
use aes::cipher::{BlockEncrypt, KeyInit};
use rsa::Pkcs1v15Encrypt;
use rsa::pkcs8::EncodePublicKey;
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha1::{Digest, Sha1};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from cryptographic operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CryptoError {
    /// RSA key generation failed.
    #[error("RSA key generation failed: {0}")]
    KeyGeneration(String),

    /// RSA decryption failed (bad shared secret or challenge).
    #[error("RSA decryption failed: {0}")]
    Decryption(String),

    /// The decrypted shared secret has the wrong length.
    #[error("invalid shared secret length: expected 16, got {0}")]
    InvalidSecretLength(usize),

    /// RSA public key encoding failed.
    #[error("public key encoding failed: {0}")]
    PublicKeyEncoding(String),
}

// ---------------------------------------------------------------------------
// RSA Key Pair
// ---------------------------------------------------------------------------

/// An RSA key pair for the Minecraft login handshake.
///
/// Generated once at server startup. The public key is sent to clients
/// in `ClientboundHelloPacket`; the private key decrypts the client's
/// shared secret.
pub struct ServerKeyPair {
    private_key: RsaPrivateKey,
    public_key: RsaPublicKey,
    /// DER-encoded public key (X.509 SubjectPublicKeyInfo) sent to clients.
    public_key_der: Vec<u8>,
}

impl ServerKeyPair {
    /// Generates a new 1024-bit RSA key pair.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError::KeyGeneration`] if the OS RNG or RSA
    /// generation fails.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut rng = rsa::rand_core::OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 1024)
            .map_err(|e| CryptoError::KeyGeneration(e.to_string()))?;
        let public_key = RsaPublicKey::from(&private_key);
        let public_key_der = public_key
            .to_public_key_der()
            .map_err(|e| CryptoError::PublicKeyEncoding(e.to_string()))?
            .to_vec();

        Ok(Self {
            private_key,
            public_key,
            public_key_der,
        })
    }

    /// Returns the DER-encoded public key (X.509 SubjectPublicKeyInfo).
    pub fn public_key_der(&self) -> &[u8] {
        &self.public_key_der
    }

    /// Returns a reference to the RSA public key.
    pub fn public_key(&self) -> &RsaPublicKey {
        &self.public_key
    }

    /// Decrypts data encrypted with the public key (PKCS#1 v1.5 padding).
    ///
    /// Used to decrypt both the shared secret and the verification challenge.
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError::Decryption`] if decryption fails.
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.private_key
            .decrypt(Pkcs1v15Encrypt, ciphertext)
            .map_err(|e| CryptoError::Decryption(e.to_string()))
    }

    /// Decrypts the client's shared secret and validates its length.
    ///
    /// The shared secret must be exactly 16 bytes (128-bit AES key).
    ///
    /// # Errors
    ///
    /// Returns [`CryptoError`] if decryption fails or the decrypted
    /// secret is not 16 bytes.
    pub fn decrypt_shared_secret(&self, encrypted_secret: &[u8]) -> Result<[u8; 16], CryptoError> {
        let decrypted = self.decrypt(encrypted_secret)?;
        decrypted
            .try_into()
            .map_err(|v: Vec<u8>| CryptoError::InvalidSecretLength(v.len()))
    }
}

impl std::fmt::Debug for ServerKeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerKeyPair")
            .field("public_key_der_len", &self.public_key_der.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// AES-128-CFB8 Cipher
// ---------------------------------------------------------------------------

/// Stateful AES-128-CFB8 stream cipher for Minecraft protocol encryption.
///
/// After the login handshake, all traffic is encrypted with AES-128-CFB8
/// where the key and IV are both the 16-byte shared secret. The cipher
/// is **stateful** — its internal feedback register advances with every
/// byte, so encryption must happen on the raw TCP byte stream, not
/// per-frame.
///
/// CFB-8 processes one byte at a time:
/// 1. Encrypt the 16-byte shift register with AES-128-ECB
/// 2. XOR the first byte of the result with the plaintext/ciphertext byte
/// 3. Shift the register left by 1 byte, inserting the output byte at the end
pub struct CipherState {
    cipher: Aes128,
    enc_iv: [u8; 16],
    dec_iv: [u8; 16],
}

impl CipherState {
    /// Creates a new cipher from the shared secret.
    ///
    /// Per the Minecraft protocol, the key and IV are both the 16-byte
    /// shared secret (AES-128-CFB8 with key == IV).
    pub fn new(shared_secret: &[u8; 16]) -> Self {
        Self {
            cipher: Aes128::new(shared_secret.into()),
            enc_iv: *shared_secret,
            dec_iv: *shared_secret,
        }
    }

    /// Decrypts data in-place.
    ///
    /// Must be called on the raw byte stream (before frame decoding).
    pub fn decrypt(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            let mut block = self.dec_iv.into();
            self.cipher.encrypt_block(&mut block);
            let ciphertext_byte = *byte;
            *byte ^= block[0];
            // Shift register: drop first byte, append ciphertext byte
            self.dec_iv.copy_within(1.., 0);
            self.dec_iv[15] = ciphertext_byte;
        }
    }

    /// Encrypts data in-place.
    ///
    /// Must be called on the raw byte stream (after frame encoding).
    pub fn encrypt(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            let mut block = self.enc_iv.into();
            self.cipher.encrypt_block(&mut block);
            *byte ^= block[0];
            // Shift register: drop first byte, append ciphertext byte
            self.enc_iv.copy_within(1.., 0);
            self.enc_iv[15] = *byte;
        }
    }
}

impl std::fmt::Debug for CipherState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CipherState").finish()
    }
}

// ---------------------------------------------------------------------------
// Minecraft Auth Hash
// ---------------------------------------------------------------------------

/// Computes the Minecraft authentication hash (server ID digest).
///
/// This is the non-standard SHA-1 "twos-complement hex" encoding used by
/// Mojang's session server. It computes:
///
/// ```text
/// SHA1(server_id_bytes + shared_secret + public_key_der)
/// ```
///
/// Then interprets the result as a signed big-endian integer and formats
/// it as a lowercase hex string (with leading minus for negative values,
/// no leading zeros).
///
/// Known test vectors from wiki.vg:
/// - `"Notch"` → `"4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48"`
/// - `"jeb_"` → `"-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1"`
/// - `"simon"` → `"88e16a1019277b15b58571f3c71afe77e69d0bda"`
pub fn minecraft_digest(server_id: &str, shared_secret: &[u8], public_key_der: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key_der);
    let hash = hasher.finalize();

    // Interpret the 20-byte hash as a signed big-endian integer.
    // If the high bit is set, the value is negative (twos-complement).
    let negative = hash[0] & 0x80 != 0;

    if negative {
        // Negate: invert all bits, add 1
        let mut bytes = hash.to_vec();
        let mut carry = true;
        for byte in bytes.iter_mut().rev() {
            *byte = !*byte;
            if carry {
                let (result, overflow) = byte.overflowing_add(1);
                *byte = result;
                carry = overflow;
            }
        }
        // Format as hex, strip leading zeros, prepend minus
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let trimmed = hex.trim_start_matches('0');
        format!("-{trimmed}")
    } else {
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        let trimmed = hex.trim_start_matches('0');
        if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        }
    }
}

/// Generates an offline-mode UUID v3 from a player name.
///
/// This matches the vanilla `UUIDUtil.createOfflineProfile()` which uses
/// `UUID.nameUUIDFromBytes("OfflinePlayer:<name>")`. Java's
/// `nameUUIDFromBytes` computes MD5 of the raw input bytes (no namespace
/// prefix), then sets version=3 and IETF variant bits.
pub fn offline_uuid(name: &str) -> uuid::Uuid {
    use md5::{Digest as Md5Digest, Md5};

    let input = format!("OfflinePlayer:{name}");
    let hash = Md5::digest(input.as_bytes());
    let mut bytes: [u8; 16] = hash.into();

    // Set version 3 (name-based MD5)
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    // Set variant (IETF)
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    uuid::Uuid::from_bytes(bytes)
}

// ---------------------------------------------------------------------------
// Verification challenge
// ---------------------------------------------------------------------------

/// Generates a random 4-byte verification challenge.
///
/// Vanilla uses `Ints.toByteArray(random.nextInt())`.
pub fn generate_challenge() -> [u8; 4] {
    use rand::RngExt;
    let mut buf = [0u8; 4];
    rand::rng().fill(&mut buf[..]);
    buf
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use rsa::rand_core::OsRng;

    // -----------------------------------------------------------------------
    // AES-128-CFB8 cipher tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cipher_encrypt_decrypt_roundtrip() {
        let secret = [0x42u8; 16];
        let mut cipher_enc = CipherState::new(&secret);
        let mut cipher_dec = CipherState::new(&secret);

        let original = b"Hello, Minecraft!".to_vec();
        let mut data = original.clone();

        cipher_enc.encrypt(&mut data);
        assert_ne!(data, original, "encrypted data should differ from original");

        cipher_dec.decrypt(&mut data);
        assert_eq!(data, original, "decrypted data should match original");
    }

    #[test]
    fn test_cipher_is_stateful() {
        // Encrypting the same plaintext twice should produce different ciphertext
        // because the cipher's feedback register advances.
        let secret = [0xAB; 16];
        let mut cipher = CipherState::new(&secret);

        let mut data1 = b"test".to_vec();
        cipher.encrypt(&mut data1);

        let mut data2 = b"test".to_vec();
        cipher.encrypt(&mut data2);

        assert_ne!(
            data1, data2,
            "same plaintext encrypted twice should differ (stateful cipher)"
        );
    }

    #[test]
    fn test_cipher_multi_chunk_roundtrip() {
        // Simulate stream: encrypt in chunks, decrypt in chunks
        let secret = [0x13; 16];
        let mut enc = CipherState::new(&secret);
        let mut dec = CipherState::new(&secret);

        let chunk1 = b"first chunk ".to_vec();
        let chunk2 = b"second chunk".to_vec();

        let mut enc1 = chunk1.clone();
        enc.encrypt(&mut enc1);

        let mut enc2 = chunk2.clone();
        enc.encrypt(&mut enc2);

        dec.decrypt(&mut enc1);
        dec.decrypt(&mut enc2);

        assert_eq!(enc1, chunk1);
        assert_eq!(enc2, chunk2);
    }

    #[test]
    fn test_cipher_empty_data() {
        let secret = [0x00; 16];
        let mut cipher = CipherState::new(&secret);
        let mut data = Vec::new();
        cipher.encrypt(&mut data); // should not panic
        cipher.decrypt(&mut data); // should not panic
    }

    // -----------------------------------------------------------------------
    // RSA key pair tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_rsa_keygen_and_der() {
        let keypair = ServerKeyPair::generate().expect("key generation");
        // RSA-1024 DER-encoded public key should be ~162 bytes
        assert!(
            keypair.public_key_der().len() > 100,
            "public key DER should be > 100 bytes"
        );
        assert!(
            keypair.public_key_der().len() < 300,
            "public key DER should be < 300 bytes"
        );
    }

    #[test]
    fn test_rsa_encrypt_decrypt_roundtrip() {
        let keypair = ServerKeyPair::generate().expect("key generation");
        let plaintext = b"shared_secret!!!"; // 16 bytes

        // Encrypt with public key
        let ciphertext = keypair
            .public_key
            .encrypt(&mut OsRng, Pkcs1v15Encrypt, plaintext)
            .expect("encryption");

        // Decrypt with private key
        let decrypted = keypair.decrypt(&ciphertext).expect("decryption");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_shared_secret_correct_length() {
        let keypair = ServerKeyPair::generate().expect("key generation");
        let secret = [0x42u8; 16];

        let encrypted = keypair
            .public_key
            .encrypt(&mut OsRng, Pkcs1v15Encrypt, &secret)
            .expect("encryption");

        let decrypted = keypair
            .decrypt_shared_secret(&encrypted)
            .expect("decryption");
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn test_decrypt_shared_secret_wrong_length() {
        let keypair = ServerKeyPair::generate().expect("key generation");
        let wrong = [0x42u8; 8]; // Only 8 bytes, not 16

        let encrypted = keypair
            .public_key
            .encrypt(&mut OsRng, Pkcs1v15Encrypt, &wrong)
            .expect("encryption");

        let err = keypair.decrypt_shared_secret(&encrypted).unwrap_err();
        assert!(matches!(err, CryptoError::InvalidSecretLength(8)));
    }

    // -----------------------------------------------------------------------
    // Auth hash tests (wiki.vg test vectors)
    // -----------------------------------------------------------------------

    #[test]
    fn test_minecraft_digest_notch() {
        // wiki.vg test vector: "Notch" → positive hash
        let result = minecraft_digest("Notch", &[], &[]);
        assert_eq!(result, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
    }

    #[test]
    fn test_minecraft_digest_jeb() {
        // wiki.vg test vector: "jeb_" → negative hash (leading minus)
        let result = minecraft_digest("jeb_", &[], &[]);
        assert_eq!(result, "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");
    }

    #[test]
    fn test_minecraft_digest_simon() {
        // wiki.vg test vector: "simon" → positive hash
        let result = minecraft_digest("simon", &[], &[]);
        assert_eq!(result, "88e16a1019277b15d58faf0541e11910eb756f6");
    }

    // -----------------------------------------------------------------------
    // Offline UUID tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_offline_uuid_deterministic() {
        let uuid1 = offline_uuid("TestPlayer");
        let uuid2 = offline_uuid("TestPlayer");
        assert_eq!(uuid1, uuid2, "same name should produce same UUID");
    }

    #[test]
    fn test_offline_uuid_different_names() {
        let uuid1 = offline_uuid("Alice");
        let uuid2 = offline_uuid("Bob");
        assert_ne!(
            uuid1, uuid2,
            "different names should produce different UUIDs"
        );
    }

    #[test]
    fn test_offline_uuid_is_v3() {
        let uuid = offline_uuid("Steve");
        assert_eq!(
            uuid.get_version(),
            Some(uuid::Version::Md5),
            "offline UUID should be version 3 (MD5)"
        );
    }

    #[test]
    fn test_offline_uuid_matches_java() {
        // Java: UUID.nameUUIDFromBytes("OfflinePlayer:Notch".getBytes())
        let uuid = offline_uuid("Notch");
        assert_eq!(
            uuid.to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f",
            "offline UUID should match Java's UUID.nameUUIDFromBytes"
        );
    }

    // -----------------------------------------------------------------------
    // Challenge generation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_challenge_length() {
        let challenge = generate_challenge();
        assert_eq!(challenge.len(), 4);
    }

    #[test]
    fn test_generate_challenge_random() {
        let c1 = generate_challenge();
        let c2 = generate_challenge();
        // Technically could be equal, but astronomically unlikely
        assert_ne!(c1, c2, "two challenges should almost certainly differ");
    }
}
