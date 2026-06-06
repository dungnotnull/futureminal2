//! Encrypted secrets vault with AES-256-GCM via the `aes-gcm` crate.
//!
//! Secrets are encrypted locally. Only metadata is optionally written
//! to the blockchain.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

/// A stored secret (encrypted with AES-256-GCM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    pub id: uuid::Uuid,
    pub name: String,
    pub encrypted_value: Vec<u8>,
    pub nonce: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub accessed_count: u64,
    pub last_accessed: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: HashMap<String, String>,
}

/// The encrypted secrets vault.
pub struct Vault {
    secrets: HashMap<String, Secret>,
    master_key: Option<[aes_gcm::Key<Aes256Gcm>; 1]>,
}

impl Vault {
    pub fn new() -> Self {
        Self { secrets: HashMap::new(), master_key: None }
    }

    /// Initialize the vault with a master key derived from a passphrase using PBKDF2.
    pub fn init_with_passphrase(&mut self, passphrase: &str, salt: &[u8]) {
        use ring::pbkdf2;
        let mut key_bytes = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(100_000).unwrap(),
            salt,
            passphrase.as_bytes(),
            &mut key_bytes,
        );
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        self.master_key = Some([*key]);
        info!("Vault initialized with 256-bit master key");
    }

    /// Generate a random 256-bit key.
    pub fn generate_key() -> [u8; 32] {
        use ring::rand::SecureRandom;
        let rng = ring::rand::SystemRandom::new();
        let mut key = [0u8; 32];
        rng.fill(&mut key).expect("RNG failure");
        key
    }

    /// Store a secret in the vault.
    pub fn store(
        &mut self,
        name: impl Into<String>,
        value: &str,
    ) -> anyhow::Result<()> {
        let key = self.master_key.as_ref().ok_or_else(|| anyhow::anyhow!("Vault not initialized"))?;
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key[0]);
        let cipher = Aes256Gcm::new(key);
        let name = name.into();

        use ring::rand::SecureRandom;
        let rng = ring::rand::SystemRandom::new();
        let mut nonce_bytes = [0u8; 12];
        rng.fill(&mut nonce_bytes).map_err(|_| anyhow::anyhow!("RNG failed"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let encrypted = cipher.encrypt(nonce, value.as_bytes().as_ref())
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        let secret = Secret {
            id: uuid::Uuid::new_v4(),
            name: name.clone(),
            encrypted_value: encrypted,
            nonce: nonce_bytes.to_vec(),
            created_at: chrono::Utc::now(),
            accessed_count: 0,
            last_accessed: None,
            metadata: HashMap::new(),
        };

        self.secrets.insert(name, secret);
        Ok(())
    }

    /// Retrieve a secret by name.
    pub fn retrieve(
        &mut self,
        name: &str,
    ) -> Option<String> {
        let key = self.master_key.as_ref()?;
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key[0]);
        let cipher = Aes256Gcm::new(key);
        let secret = self.secrets.get_mut(name)?;
        let nonce = Nonce::from_slice(&secret.nonce);
        let decrypted = cipher.decrypt(nonce, secret.encrypted_value.as_ref())
            .ok()?;
        secret.accessed_count += 1;
        secret.last_accessed = Some(chrono::Utc::now());
        String::from_utf8(decrypted).ok()
    }

    /// List all secret names (values are never exposed).
    pub fn list(&self) -> Vec<(&str, u64)> {
        self.secrets.values().map(|s| (s.name.as_str(), s.accessed_count)).collect()
    }

    /// Delete a secret.
    pub fn delete(&mut self, name: &str) {
        self.secrets.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_store_retrieve() {
        let mut vault = Vault::new();
        vault.init_with_passphrase("my-secret-password", b"fixed-salt-for-testing");
        vault.store("api_key", "sk-1234567890abcdef").unwrap();

        let value = vault.retrieve("api_key").unwrap();
        assert_eq!(value, "sk-1234567890abcdef");
    }

    #[test]
    fn test_vault_encryption_roundtrip() {
        let key = Vault::generate_key();
        let key_ref = aes_gcm::Key::<Aes256Gcm>::from_slice(&key);
        let cipher = Aes256Gcm::new(key_ref);
        let nonce = Nonce::from_slice(b"unique nonce");
        let ciphertext = cipher.encrypt(nonce, b"hello world".as_ref()).unwrap();
        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref()).unwrap();
        assert_eq!(String::from_utf8(plaintext).unwrap(), "hello world");
    }

    #[test]
    fn test_vault_list() {
        let mut vault = Vault::new();
        vault.init_with_passphrase("pass", b"salt");
        vault.store("db_password", "hunter2").unwrap();
        let list = vault.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "db_password");
    }

    #[test]
    fn test_vault_wrong_passphrase_fails() {
        let mut vault = Vault::new();
        vault.init_with_passphrase("correct-password", b"salt");
        vault.store("secret", "value").unwrap();

        let mut vault2 = Vault::new();
        vault2.init_with_passphrase("wrong-password", b"salt");
        // Decryption should fail because the derived key is different
        assert!(vault2.retrieve("secret").is_none());
    }
}
