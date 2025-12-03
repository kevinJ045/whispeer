//! # Whispeer
//!
//! `whispeer` is a Rust-native, lightweight, end-to-end **encrypted + compressed Pub/Sub broker** designed for secure messaging, real-time applications, and extensible event-driven systems.
//!
//! ## Core Features
//! 1. **Broker Core**
//!    - Topic registry
//!    - Subscriber management
//!    - Publish/subscribe API
//! 2. **Compression Engine**
//!    - Default: zstd
//! 3. **Encryption Engine**
//!    - Default: ChaCha20Poly1305
//! 4. **Transport**
//!    - QUIC-based transport for secure and efficient communication.
//! 5. **Async Support**
//!    - Built on the Tokio runtime.
//!    - Async subscribers.
//! 6. **Plugin System**
//!    - Easily extend functionality with custom plugins.
//! 7. **Typed Messages**
//!    - Support for generic message types via `serde::Serialize` and `serde::Deserialize`.
//!
//! ## Simple Example
//!
//! ```rust,no_run
//! use whispeer::Broker;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
//! struct MyData {
//!   name: String,
//!   id: i32
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let broker = Broker::new();
//!     let topic = "some-event";
//!
//!     // Subscribe to a topic with a specific type
//!     broker.subscribe(topic, |data: MyData| {
//!         Box::pin(async move {
//!             println!("Received data: {:?}", data);
//!         })
//!     });
//!
//!     // Publish data to the topic
//!     let my_data = MyData {
//!         name: "myname".to_string(),
//!         id: 123
//!     };
//!
//!     if let Err(e) = broker.publish(topic, my_data).await {
//!         eprintln!("Failed to publish: {}", e);
//!     }
//! }
//! ```

pub mod broker;
pub mod plugins;
pub mod transport;

pub use broker::Broker;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::plugins::compression::CompressionPlugin;
  use crate::plugins::encryption::EncryptionPlugin;
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::sync::Mutex;

  #[tokio::test]
  async fn test_pub_sub() {
    let broker = Broker::new();
    let topic_name = "test_topic";

    let received_msg = Arc::new(Mutex::new(None));
    let received_msg_clone = received_msg.clone();

    broker.subscribe(topic_name, move |msg: String| {
      let received_msg = received_msg_clone.clone();
      Box::pin(async move {
        let mut lock = received_msg.lock().await;
        *lock = Some(msg);
      })
    });

    let msg = "Hello, World!".to_string();
    broker.publish(topic_name, msg.clone()).await.unwrap();

    let lock = received_msg.lock().await;
    assert_eq!(*lock, Some(msg));
  }

  #[tokio::test]
  async fn test_multiple_types() {
    let broker = Broker::new();

    // String topic
    broker.subscribe("string_topic", |msg: String| {
      Box::pin(async move {
        println!("Received string: {}", msg);
      })
    });

    // i32 topic
    broker.subscribe("int_topic", |msg: i32| {
      Box::pin(async move {
        println!("Received int: {}", msg);
      })
    });

    broker
      .publish("string_topic", "hello".to_string())
      .await
      .unwrap();
    broker.publish("int_topic", 42).await.unwrap();

    // Type mismatch check
    let res = broker.publish("string_topic", 123).await;
    assert!(res.is_err());
  }

  #[tokio::test]
  async fn test_p2p_messaging() {
    // Broker A - Listener
    let addr_a = "127.0.0.1:5002";
    let broker_a = Broker::start(format!("server {}", addr_a))
      .await
      .expect("Failed to start broker A");

    // Broker B - Connector
    // We give it the same address so it tries to bind, fails, and connects to A
    let broker_b = Broker::start(format!("client {}", addr_a))
      .await
      .expect("Failed to start broker B");

    // Subscribe on A
    let received_msg = Arc::new(Mutex::new(None));
    let received_msg_clone = received_msg.clone();
    broker_a.subscribe("chat", move |msg: String| {
      let received_msg = received_msg_clone.clone();
      Box::pin(async move {
        let mut lock = received_msg.lock().await;
        *lock = Some(msg);
      })
    });

    // Wait for connection to be established
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Publish on B
    let msg = "Hello from B".to_string();
    broker_b
      .publish("chat", msg.clone())
      .await
      .expect("Failed to publish");

    // Wait for message delivery
    tokio::time::sleep(Duration::from_secs(1)).await;

    let lock = received_msg.lock().await;
    assert_eq!(*lock, Some(msg));
  }

  #[tokio::test]
  async fn test_compression_plugin() {
    // Broker A - Listener
    let addr_a = "127.0.0.1:5005";
    let broker_a = Broker::start(format!("server {}", addr_a))
      .await
      .expect("Failed to start broker A");
    broker_a.add_plugin(CompressionPlugin::new()).await;

    // Broker B - Connector
    let broker_b = Broker::start(format!("client {}", addr_a))
      .await
      .expect("Failed to start broker B");
    broker_b.add_plugin(CompressionPlugin::new()).await;

    // Subscribe on A
    let received_msg = Arc::new(Mutex::new(None));
    let received_msg_clone = received_msg.clone();
    broker_a.subscribe("compressed_chat", move |msg: String| {
      let received_msg = received_msg_clone.clone();
      Box::pin(async move {
        let mut lock = received_msg.lock().await;
        *lock = Some(msg);
      })
    });

    // Wait for connection
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Publish on B
    let msg = "This is a message that should be compressed".to_string();
    broker_b
      .publish("compressed_chat", msg.clone())
      .await
      .expect("Failed to publish");

    // Wait for message delivery
    tokio::time::sleep(Duration::from_secs(1)).await;

    let lock = received_msg.lock().await;
    assert_eq!(*lock, Some(msg));
  }

  #[tokio::test]
  async fn test_encryption_plugin() {
    // Broker A - Listener
    let addr_a = "127.0.0.1:5003";
    let encryption_key = [
      12, 22, 44, 55, 66, 66, 33, 44, 55, 66, 77, 88, 99, 100, 111, 122, 133, 144, 155, 166, 177,
      188, 199, 200, 211, 222, 233, 244, 4, 55, 66, 44,
    ];
    let broker_a = Broker::start(format!("server {}", addr_a))
      .await
      .expect("Failed to start broker A");
    broker_a
      .add_plugin(EncryptionPlugin::new(encryption_key.clone()))
      .await;

    // Broker B - Connector
    let broker_b = Broker::start(format!("client {}", addr_a))
      .await
      .expect("Failed to start broker B");
    broker_b
      .add_plugin(EncryptionPlugin::new(encryption_key.clone()))
      .await;

    // Subscribe on A
    let received_msg = Arc::new(Mutex::new(None));
    let received_msg_clone = received_msg.clone();
    broker_a.subscribe("encrypted", move |msg: String| {
      let received_msg = received_msg_clone.clone();
      Box::pin(async move {
        let mut lock = received_msg.lock().await;
        *lock = Some(msg);
      })
    });

    // Wait for connection
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Publish on B
    let msg = "This is a message that should be encrypted".to_string();
    broker_b
      .publish("encrypted", msg.clone())
      .await
      .expect("Failed to publish");

    // Wait for message delivery
    tokio::time::sleep(Duration::from_secs(1)).await;

    let lock = received_msg.lock().await;
    assert_eq!(*lock, Some(msg));
  }

  #[tokio::test]
  async fn test_p2p_types() {
    use serde::{Deserialize, Serialize};
    // Broker A - Listener
    let addr_a = "127.0.0.1:5004"; // Changed to avoid conflict with test_p2p_messaging
    let broker_a = Broker::start(format!("server {addr_a}"))
      .await
      .expect("Failed to start broker A");

    // Message to send
    #[derive(Debug, Serialize, Deserialize, Clone)]
    enum ChatMessage {
      Loading,
      Message { name: String, content: String },
    }

    // Not Needed, Only here for the test
    impl PartialEq for ChatMessage {
      fn eq(&self, other: &Self) -> bool {
        match self {
          ChatMessage::Loading => matches!(other, ChatMessage::Loading),
          ChatMessage::Message {
            name: selfname,
            content: selfcontent,
          } => match other {
            ChatMessage::Loading => false,
            ChatMessage::Message { name, content } => selfname == name && selfcontent == content,
          },
        }
      }
    }

    // Broker B - Connector
    let broker_b = Broker::start(format!("client {addr_a}"))
      .await
      .expect("Failed to start broker B");

    // Subscribe on A for ChatMessage
    let received_chat_msg = Arc::new(Mutex::new(None));
    let received_chat_msg_clone = received_chat_msg.clone();
    broker_a.subscribe("chat", move |msg: ChatMessage| {
      println!("{msg:?}");
      let received_msg = received_chat_msg_clone.clone();
      Box::pin(async move {
        let mut lock = received_msg.lock().await;
        *lock = Some(msg);
      })
    });

    // Subscribe on A for i32
    let received_int_msg = Arc::new(Mutex::new(None));
    let received_int_msg_clone = received_int_msg.clone();
    broker_a.subscribe("numbers", move |msg: i32| {
      println!("Received int: {msg}");
      let received_msg = received_int_msg_clone.clone();
      Box::pin(async move {
        let mut lock = received_msg.lock().await;
        *lock = Some(msg);
      })
    });

    // Wait for connection to be established
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Publish ChatMessage on B
    let chat_msg = ChatMessage::Message {
      name: "Somedude".to_string(),
      content: "Hello!".to_string(),
    };
    broker_b
      .publish("chat", chat_msg.clone())
      .await
      .expect("Failed to publish");

    // Publish i32 on B
    let int_msg = 42;
    broker_b
      .publish("numbers", int_msg)
      .await
      .expect("Failed to publish");

    // Wait for message delivery
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Assert ChatMessage
    let lock = received_chat_msg.lock().await;
    assert_eq!(*lock, Some(chat_msg));

    // Assert i32
    let lock = received_int_msg.lock().await;
    assert_eq!(*lock, Some(int_msg));
  }
}
