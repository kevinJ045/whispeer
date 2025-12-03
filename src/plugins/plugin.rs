use async_trait::async_trait;
use std::collections::HashMap;

/// Defines the interface for a Whispeer broker plugin.
///
/// Plugins allow for extending the broker's functionality by hooking into its lifecycle events.
/// Each method in this trait corresponds to a specific event.
#[async_trait]
pub trait Plugin: Send + Sync {
  /// Returns the name of the plugin.
  fn name(&self) -> &str;

  /// Called when the plugin is initialized.
  ///
  /// This method can be used to perform any setup required by the plugin.
  ///
  /// # Arguments
  ///
  /// * `_broker` - A reference to the broker instance.
  async fn on_init(&mut self, _broker: &crate::Broker) -> Result<(), anyhow::Error> {
    Ok(())
  }

  /// Called before a message is published.
  ///
  /// This method can be used to modify the message payload or headers before it is sent.
  /// This is a **required** method.
  ///
  /// # Arguments
  ///
  /// * `topic` - The topic the message is being published to.
  /// * `payload` - A mutable reference to the message payload.
  /// * `headers` - A mutable reference to the message headers.
  async fn on_publish(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &mut HashMap<String, String>,
  ) -> Result<(), anyhow::Error>;

  /// Called after a message is received from the network but before it is delivered to subscribers.
  ///
  /// This method can be used to modify the message payload based on its headers.
  /// This is a **required** method.
  ///
  /// # Arguments
  ///
  /// * `topic` - The topic of the received message.
  /// * `payload` - A mutable reference to the message payload.
  /// * `headers` - A reference to the message headers.
  async fn on_message_received(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error>;

  /// Called before a message is received by the broker.
  ///
  /// This method can be used to transform the topic of a message before it is handled.
  ///
  /// # Arguments
  ///
  /// * `topic` - The original topic of the message.
  /// * `_payload` - A mutable reference to the message payload.
  /// * `_headers` - A mutable reference to the message headers.
  async fn on_before_recieved(
    &self,
    topic: &str,
    _payload: &mut Vec<u8>,
    _headers: &mut HashMap<String, String>,
  ) -> Result<String, anyhow::Error> {
    Ok(topic.to_string())
  }

  /// Called when a new subscription is made.
  ///
  /// # Arguments
  ///
  /// * `_topic` - The topic that was subscribed to.
  async fn on_subscribe(&self, _topic: &str) -> Result<(), anyhow::Error> {
    Ok(())
  }
}
