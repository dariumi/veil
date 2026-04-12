use crate::error::{Result, VeilError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{
    aead, digest, hmac,
    rand::{SecureRandom, SystemRandom},
};

pub mod token;
pub use token::{AccessToken, TokenManager};

/// Generate cryptographically secure random bytes
pub fn random_bytes(len: usize) -> Vec<u8> {
    let rng = SystemRandom::new();
    let mut buf = vec![0u8; len];
    rng.fill(&mut buf).expect("RNG failure");
    buf
}

/// Generate a URL-safe base64 token of given byte length
pub fn generate_token(byte_len: usize) -> String {
    let bytes = random_bytes(byte_len);
    URL_SAFE_NO_PAD.encode(&bytes)
}

/// HMAC-SHA256 for challenge/response auth
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, data).as_ref().to_vec()
}

/// Verify HMAC-SHA256
pub fn verify_hmac_sha256(key: &[u8], data: &[u8], expected: &[u8]) -> bool {
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::verify(&key, data, expected).is_ok()
}

/// SHA-256 digest
pub fn sha256(data: &[u8]) -> Vec<u8> {
    digest::digest(&digest::SHA256, data).as_ref().to_vec()
}

/// AEAD encryption using AES-256-GCM
pub struct AeadCipher {
    key: aead::LessSafeKey,
}

impl AeadCipher {
    pub fn new(key_bytes: &[u8]) -> Result<Self> {
        let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes)
            .map_err(|_| VeilError::Crypto("Invalid key length".into()))?;
        Ok(Self {
            key: aead::LessSafeKey::new(unbound),
        })
    }

    pub fn encrypt(&self, nonce_bytes: &[u8; 12], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce = aead::Nonce::assume_unique_for_key(*nonce_bytes);
        let aad = aead::Aad::from(aad);
        let mut in_out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(nonce, aad, &mut in_out)
            .map_err(|_| VeilError::Crypto("Encryption failed".into()))?;
        Ok(in_out)
    }

    pub fn decrypt(
        &self,
        nonce_bytes: &[u8; 12],
        aad: &[u8],
        ciphertext: &[u8],
    ) -> Result<Vec<u8>> {
        let nonce = aead::Nonce::assume_unique_for_key(*nonce_bytes);
        let aad = aead::Aad::from(aad);
        let mut in_out = ciphertext.to_vec();
        let plaintext = self
            .key
            .open_in_place(nonce, aad, &mut in_out)
            .map_err(|_| {
                VeilError::Crypto("Decryption failed / authentication tag mismatch".into())
            })?;
        Ok(plaintext.to_vec())
    }
}
