use crate::broker::subscriber::Subscriber;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::any::Any;

#[async_trait]
pub trait TopicOperations: Send + Sync {
    async fn publish_json(&self, payload: &str);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct Topic<T> {
    pub name: String,
    pub subscribers: Vec<Subscriber<T>>,
}

#[async_trait]
impl<T: DeserializeOwned + Clone + Send + Sync + 'static> TopicOperations for Topic<T> {
    async fn publish_json(&self, payload: &str) {
        if let Ok(message) = serde_json::from_str::<T>(payload) {
            for sub in &self.subscribers {
                let msg_clone = message.clone();
                (sub.handler)(msg_clone).await;
            }
        } else {
            eprintln!("Failed to deserialize payload for topic {}", self.name);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<T> Topic<T> {
    pub fn new(name: String) -> Self {
        Self {
            name,
            subscribers: Vec::new(),
        }
    }

    pub fn add_subscriber(&mut self, subscriber: Subscriber<T>) {
        self.subscribers.push(subscriber);
    }

    pub fn remove_subscriber(&mut self, subscriber_id: uuid::Uuid) {
        self.subscribers.retain(|s| s.id != subscriber_id);
    }
}
