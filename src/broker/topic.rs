use crate::broker::subscriber::Subscriber;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::any::Any;

/// A trait that provides a type-erased interface for topic operations.
///
/// This allows the broker to manage topics of different generic types through a
/// common interface. It is particularly useful for operations that do not depend
/// on the message type, such as publishing a raw JSON payload.
#[async_trait]
pub trait TopicOperations: Send + Sync {
  /// Publishes a JSON payload to the topic.
  ///
  /// The payload is deserialized into the topic's specific message type and then
  /// sent to all subscribers.
  async fn publish_json(&self, payload: &str);

  /// Subscribes to the topic with a JSON handler.
  ///
  /// The handler receives the raw JSON string.
  fn subscribe_json(
    &mut self,
    handler: Box<
      dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
    >,
  );

  /// Returns the topic as a `&dyn Any` to allow for downcasting.
  fn as_any(&self) -> &dyn Any;

  /// Returns the topic as a `&mut dyn Any` to allow for mutable downcasting.
  fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Represents a pub/sub topic.
///
/// A `Topic` manages a list of subscribers for a specific message type `T`.
/// It is responsible for dispatching messages to all of its subscribers.
pub struct Topic<T> {
  /// The name of the topic.
  pub name: String,
  /// A list of subscribers with typed handlers.
  pub subscribers: Vec<Subscriber<T>>,
  /// A list of subscribers with raw JSON string handlers.
  pub json_subscribers: Vec<
    Box<
      dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
    >,
  >,
}

#[async_trait]
impl<T: DeserializeOwned + Serialize + Clone + Send + Sync + 'static> TopicOperations for Topic<T> {
  async fn publish_json(&self, payload: &str) {
    if let Ok(message) = serde_json::from_str::<T>(payload) {
      for sub in &self.subscribers {
        let msg_clone = message.clone();
        (sub.handler)(msg_clone).await;
      }

      let payload_string = payload.to_string();
      for sub in &self.json_subscribers {
        (sub)(payload_string.clone()).await;
      }
    } else {
      eprintln!("Failed to deserialize payload for topic {}", self.name);
    }
  }

  fn subscribe_json(
    &mut self,
    handler: Box<
      dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
    >,
  ) {
    self.json_subscribers.push(handler);
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

impl<T> Topic<T> {
  /// Creates a new `Topic` with a given name.
  ///
  /// # Arguments
  ///
  /// * `name` - The name of the topic.
  pub fn new(name: String) -> Self {
    Self {
      name,
      subscribers: Vec::new(),
      json_subscribers: Vec::new(),
    }
  }

  /// Adds a new subscriber to the topic.
  ///
  /// # Arguments
  ///
  /// * `subscriber` - The `Subscriber` to add.
  pub fn add_subscriber(&mut self, subscriber: Subscriber<T>) {
    self.subscribers.push(subscriber);
  }

  /// Removes a subscriber from the topic by its ID.
  ///
  /// # Arguments
  ///
  /// * `subscriber_id` - The ID of the subscriber to remove.
  pub fn remove_subscriber(&mut self, subscriber_id: uuid::Uuid) {
    self.subscribers.retain(|s| s.id != subscriber_id);
  }
}
