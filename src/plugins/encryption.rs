use crate::plugins::plugin::Plugin;
use async_trait::async_trait;
use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit}; // <-- correct import
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

pub struct EncryptionPlugin {
  key: [u8; 32],
}

impl EncryptionPlugin {
  pub fn new(key: [u8; 32]) -> Self {
    Self { key }
  }
}

#[async_trait]
impl Plugin for EncryptionPlugin {
  fn name(&self) -> &str {
    "EncryptionPlugin"
  }

  async fn on_publish(
    &self,
    _topic: &str,
    payload: &mut Vec<u8>,
    headers: &mut HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));

    // Generate a random nonce (12 bytes)
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let encrypted = cipher
      .encrypt(nonce, payload.as_ref())
      .map_err(|e| anyhow::anyhow!("encryption failed: {}", e))?;

    *payload = encrypted;

    // Add headers
    headers.insert("X-Encryption".into(), "aes256-gcm".into());
    headers.insert("X-Nonce".into(), base64::encode(nonce_bytes));

    Ok(())
  }

  async fn on_message_received(
    &self,
    _topic: &str,
    payload: &mut Vec<u8>,
    headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    if headers.get("X-Encryption").map(String::as_str) != Some("aes256-gcm") {
      return Ok(());
    }

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));

    // Read nonce from headers
    let nonce_b64 = headers
      .get("X-Nonce")
      .ok_or_else(|| anyhow::anyhow!("missing nonce header"))?;

    let nonce_bytes = base64::decode(nonce_b64)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Decrypt
    let decrypted = cipher
      .decrypt(nonce, payload.as_ref())
      .map_err(|e| anyhow::anyhow!("decryption failed: {}", e))?;

    *payload = decrypted;

    Ok(())
  }
}
