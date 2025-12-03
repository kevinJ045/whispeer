use crate::plugins::plugin::Plugin;
use std::collections::HashMap;

pub struct PluginManager {
  plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
  pub fn new() -> Self {
    Self {
      plugins: Vec::new(),
    }
  }

  pub fn add_plugin(&mut self, plugin: Box<dyn Plugin>) {
    self.plugins.push(plugin);
  }

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

  pub async fn on_message_received(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    // Execute in reverse order for receiving? Or same order?
    // Usually reverse for symmetric operations (like compression/encryption).
    // If plugin A encrypts then plugin B compresses on publish (A -> B).
    // On receive, we should decompress (B) then decrypt (A) (B -> A).
    for plugin in self.plugins.iter().rev() {
      plugin.on_message_received(topic, payload, headers).await?;
    }
    Ok(())
  }

  pub async fn on_subscribe(
    &self,
    topic: &str,
  ) -> Result<(), anyhow::Error> {
    for plugin in &self.plugins {
      plugin.on_subscribe(topic).await?;
    }
    Ok(())
  }
}
