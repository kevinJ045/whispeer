use crate::plugins::plugin::Plugin;
use async_trait::async_trait;
use std::collections::HashMap;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine;
use rand::RngCore;

/// A plugin that provides end-to-end encryption for messages.
///
/// This plugin uses AES-256-GCM to encrypt message payloads before they are published
/// and decrypt them after they are received, ensuring message confidentiality.
pub struct EncryptionPlugin {
  key: [u8; 32],
}

impl EncryptionPlugin {
  /// Creates a new `EncryptionPlugin` with the given encryption key.
  ///
  /// # Arguments
  ///
  /// * `key` - A 32-byte array representing the AES-256 key.
  pub fn new(key: [u8; 32]) -> Self {
    Self { key }
  }
}

#[async_trait]
impl Plugin for EncryptionPlugin {
  fn name(&self) -> &str {
    "EncryptionPlugin"
  }

  /// Encrypts the message payload using AES-256-GCM before publishing.
  ///
  /// It generates a random nonce for each message, encrypts the payload, and then
  /// adds `X-Encryption: aes256-gcm` and `X-Nonce` headers to the message.
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

    let encrypted = cipher
      .encrypt(nonce, payload.as_ref())
      .map_err(|e| anyhow::anyhow!("encryption failed: {}", e))?;

    *payload = encrypted;

    headers.insert("X-Encryption".into(), "aes256-gcm".into());
    headers.insert(
      "X-Nonce".into(),
      base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
    );

    Ok(())
  }

  /// Decrypts the message payload if it has the appropriate encryption headers.
  ///
  /// It looks for the `X-Encryption: aes256-gcm` and `X-Nonce` headers to
  /// correctly decrypt the payload.
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

    let nonce_b64 = headers
      .get("X-Nonce")
      .ok_or_else(|| anyhow::anyhow!("missing nonce header"))?;

    let nonce_bytes = base64::engine::general_purpose::STANDARD.decode(nonce_b64)?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Decrypt
    let decrypted = cipher
      .decrypt(nonce, payload.as_ref())
      .map_err(|e| anyhow::anyhow!("decryption failed: {}", e))?;

    *payload = decrypted;

    Ok(())
  }
}
