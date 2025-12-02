use crate::broker::subscriber::Subscriber;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::any::Any;

#[async_trait]
pub trait TopicOperations: Send + Sync {
    async fn publish_json(&self, payload: &str);
    fn subscribe_json(&mut self, handler: Box<dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct Topic<T> {
    pub name: String,
    pub subscribers: Vec<Subscriber<T>>,
    pub json_subscribers: Vec<Box<dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>>,
}

#[async_trait]
impl<T: DeserializeOwned + Serialize + Clone + Send + Sync + 'static> TopicOperations for Topic<T> {
    async fn publish_json(&self, payload: &str) {
        if let Ok(message) = serde_json::from_str::<T>(payload) {
            // Notify typed subscribers
            for sub in &self.subscribers {
                let msg_clone = message.clone();
                (sub.handler)(msg_clone).await;
            }
            
            // Notify JSON subscribers
            // We can just pass the payload string directly since it's already JSON
            let payload_string = payload.to_string();
            for sub in &self.json_subscribers {
                (sub)(payload_string.clone()).await;
            }
        } else {
            eprintln!("Failed to deserialize payload for topic {}", self.name);
        }
    }

    fn subscribe_json(&mut self, handler: Box<dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>) {
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
    pub fn new(name: String) -> Self {
        Self {
            name,
            subscribers: Vec::new(),
            json_subscribers: Vec::new(),
        }
    }

    pub fn add_subscriber(&mut self, subscriber: Subscriber<T>) {
        self.subscribers.push(subscriber);
    }

    pub fn remove_subscriber(&mut self, subscriber_id: uuid::Uuid) {
        self.subscribers.retain(|s| s.id != subscriber_id);
    }
}
