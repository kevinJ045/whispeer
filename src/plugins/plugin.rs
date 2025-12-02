use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait Plugin: Send + Sync {
  fn name(&self) -> &str;

  async fn on_init(&mut self, _broker: &crate::Broker) -> Result<(), anyhow::Error> {
    Ok(())
  }

  async fn on_publish(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &mut HashMap<String, String>,
  ) -> Result<(), anyhow::Error>;

  async fn on_message_received(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error>;
}
