use crate::plugins::plugin::Plugin;
use std::collections::HashMap;

/// Manages the registered plugins and their lifecycle.
///
/// The `PluginManager` holds a list of active plugins and is responsible for invoking
/// their respective hooks at the appropriate times in the broker's lifecycle.
pub struct PluginManager {
  plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
  /// Creates a new, empty `PluginManager`.
  pub fn new() -> Self {
    Self {
      plugins: Vec::new(),
    }
  }

  /// Adds a plugin to the manager.
  ///
  /// # Arguments
  ///
  /// * `plugin` - A boxed `Plugin` trait object.
  pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) {
    self.plugins.push(plugin);
  }

  /// Invokes the `on_publish` hook on all registered plugins.
  pub async fn on_publish(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &mut HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    for plugin in &self.plugins {
      plugin.on_publish(topic, payload, headers).await?;
    }
    Ok(())
  }

  /// Invokes the `on_before_recieved` hook on all registered plugins.
  pub async fn on_before_recieve(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &mut HashMap<String, String>,
  ) -> Result<String, anyhow::Error> {
    let orig_topic = topic;
    let mut topic = topic.to_string();
    for plugin in &self.plugins {
      topic = plugin
        .on_before_recieved(orig_topic, payload, headers)
        .await?;
    }
    Ok(topic)
  }

  /// Invokes the `on_message_received` hook on all registered plugins.
  ///
  /// Note: This method executes the plugins in reverse order to ensure that symmetric
  /// operations (like compression/encryption on publish and decompression/decryption
  /// on receive) are handled correctly.
  pub async fn on_message_received(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    for plugin in self.plugins.iter().rev() {
      plugin.on_message_received(topic, payload, headers).await?;
    }
    Ok(())
  }

  /// Invokes the `on_subscribe` hook on all registered plugins.
  pub async fn on_subscribe(&self, topic: &str) -> Result<(), anyhow::Error> {
    for plugin in &self.plugins {
      plugin.on_subscribe(topic).await?;
    }
    Ok(())
  }
}
