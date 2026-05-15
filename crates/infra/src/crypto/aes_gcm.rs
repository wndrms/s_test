use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rand::RngCore;

use lumos_app::service::secret::SecretEncryptor;

pub struct AesGcmEncryptor {
    cipher: Aes256Gcm,
}

impl AesGcmEncryptor {
    /// `key_bytes` must be exactly 32 bytes (256-bit)
    pub fn new(key_bytes: &[u8; 32]) -> Self {
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        Self {
            cipher: Aes256Gcm::new(key),
        }
    }

    pub fn from_base64(b64_key: &str) -> Result<Self> {
        let bytes = B64
            .decode(b64_key)
            .context("invalid base64 encryption key")?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("encryption key must be 32 bytes"))?;
        Ok(Self::new(&arr))
    }
}

impl SecretEncryptor for AesGcmEncryptor {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("encrypt failed: {e}"))?;

        // layout: [12-byte nonce][ciphertext]
        let mut out = Vec::with_capacity(12 + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            anyhow::bail!("ciphertext too short");
        }
        let (nonce_bytes, ct) = ciphertext.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        self.cipher
            .decrypt(nonce, ct)
            .map_err(|e| anyhow::anyhow!("decrypt failed: {e}"))
    }

    fn mask(&self, raw: &str) -> String {
        let chars: Vec<char> = raw.chars().collect();
        let len = chars.len();
        if len <= 8 {
            return "*".repeat(len);
        }
        let visible = 4;
        let prefix: String = chars[..visible].iter().collect();
        let suffix: String = chars[len - visible..].iter().collect();
        format!("{}...{}", prefix, suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_encryptor() -> AesGcmEncryptor {
        AesGcmEncryptor::new(&[0x42u8; 32])
    }

    #[test]
    fn roundtrip() {
        let enc = test_encryptor();
        let plaintext = b"sk-test-api-key-1234567890";
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn mask_long_key() {
        let enc = test_encryptor();
        assert_eq!(enc.mask("sk-proj-abcdefghij1234"), "sk-p...1234");
    }

    #[test]
    fn mask_short_key() {
        let enc = test_encryptor();
        assert_eq!(enc.mask("abc"), "***");
    }
}
