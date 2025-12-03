use crate::plugins::plugin::Plugin;
use async_compression::tokio::write::{ZstdDecoder, ZstdEncoder};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::io::AsyncWriteExt;

/// A plugin that provides transparent message compression and decompression.
///
/// This plugin uses the Zstandard (zstd) algorithm to compress message payloads
/// before they are published and decompress them after they are received.
pub struct CompressionPlugin;

impl CompressionPlugin {
  /// Creates a new `CompressionPlugin`.
  pub fn new() -> Self {
    Self
  }
}

#[async_trait]
impl Plugin for CompressionPlugin {
  fn name(&self) -> &str {
    "CompressionPlugin"
  }

  /// Compresses the message payload using zstd before publishing.
  ///
  /// It also adds an `X-Compression: zstd` header to the message to indicate
  /// that the payload has been compressed.
  async fn on_publish(
    &self,
    _topic: &str,
    payload: &mut Vec<u8>,
    headers: &mut HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    // Compress
    let mut encoder = ZstdEncoder::new(Vec::new());
    encoder.write_all(payload).await?;
    encoder.shutdown().await?;
    *payload = encoder.into_inner();

    headers.insert("X-Compression".to_string(), "zstd".to_string());
    Ok(())
  }

  /// Decompresses the message payload if it has the `X-Compression: zstd` header.
  async fn on_message_received(
    &self,
    _topic: &str,
    payload: &mut Vec<u8>,
    headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    if let Some(method) = headers.get("X-Compression") {
      if method == "zstd" {
        let mut decoder = ZstdDecoder::new(Vec::new());
        decoder.write_all(payload).await?;
        decoder.shutdown().await?;
        *payload = decoder.into_inner();
      }
    }
    Ok(())
  }
}
