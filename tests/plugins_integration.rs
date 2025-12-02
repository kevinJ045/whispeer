use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use whispeer::Broker;
use whispeer::plugins::compression::CompressionPlugin;
use whispeer::plugins::encryption::EncryptionPlugin;
use whispeer::plugins::plugin::Plugin;

#[tokio::test]
async fn test_compression_and_encryption_together() {
  // Broker A - Listener
  let addr_a = "127.0.0.1:5005";
  let encryption_key = [
    12, 22, 44, 55, 66, 66, 33, 44, 55, 66, 77, 88, 99, 100, 111, 122, 133, 144, 155, 166, 177,
    188, 199, 200, 211, 222, 233, 244, 4, 55, 66, 44,
  ];

  let broker_a = Broker::start(format!("server {}", addr_a))
    .await
    .expect("Failed to start broker A");

  // Order matters! Compress then Encrypt on publish.
  // So we add Compression first, then Encryption?
  // PluginManager::on_publish iterates in order.
  // 1. Compression.on_publish (compresses payload)
  // 2. Encryption.on_publish (encrypts compressed payload)
  // This is the correct order.
  broker_a
    .add_plugin(Box::new(CompressionPlugin::new()))
    .await;
  broker_a
    .add_plugin(Box::new(EncryptionPlugin::new(encryption_key.clone())))
    .await;

  // Broker B - Connector
  let broker_b = Broker::start(format!("client {}", addr_a))
    .await
    .expect("Failed to start broker B");

  broker_b
    .add_plugin(Box::new(CompressionPlugin::new()))
    .await;
  broker_b
    .add_plugin(Box::new(EncryptionPlugin::new(encryption_key.clone())))
    .await;

  // Subscribe on A
  let received_msg = Arc::new(Mutex::new(None));
  let received_msg_clone = received_msg.clone();
  broker_a.subscribe("secure_chat", move |msg: String| {
    let received_msg = received_msg_clone.clone();
    Box::pin(async move {
      let mut lock = received_msg.lock().await;
      *lock = Some(msg);
    })
  });

  // Wait for connection
  tokio::time::sleep(Duration::from_secs(1)).await;

  // Publish on B
  let msg = "This is a secret compressed message".to_string();
  broker_b
    .publish("secure_chat", msg.clone())
    .await
    .expect("Failed to publish");

  // Wait for message delivery
  tokio::time::sleep(Duration::from_secs(1)).await;

  let lock = received_msg.lock().await;
  assert_eq!(*lock, Some(msg));
}

// Test for Plugin System Extension (on_init and broadcast)
struct EchoPlugin {
  broker: Arc<Mutex<Option<Broker>>>,
}

impl EchoPlugin {
  fn new() -> Self {
    Self {
      broker: Arc::new(Mutex::new(None)),
    }
  }
}

#[async_trait]
impl Plugin for EchoPlugin {
  fn name(&self) -> &str {
    "EchoPlugin"
  }

  async fn on_init(&mut self, broker: &Broker) -> Result<(), anyhow::Error> {
    let mut lock = self.broker.lock().await;
    broker.subscribe("ping", move |_: String| Box::pin(async move {}));
    *lock = Some(broker.clone());
    Ok(())
  }

  async fn on_publish(
    &self,
    _topic: &str,
    _payload: &mut Vec<u8>,
    _headers: &mut HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    Ok(())
  }

  async fn on_message_received(
    &self,
    topic: &str,
    payload: &mut Vec<u8>,
    _headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    if topic == "ping" {
      let broker_lock = self.broker.lock().await;
      if let Some(broker) = &*broker_lock {
        // Publish "pong" back
        // We need to be careful about infinite loops if we listen to "pong" too, but we don't here.
        // Also, payload is bytes.
        let msg_str = String::from_utf8(payload.clone()).unwrap_or_default();
        if msg_str == "\"ping\"" {
          // JSON string
          // We can't easily publish from here because we are in an async trait method
          // and we need to spawn a task or just await.
          // But we can't await publish because it might deadlock if it waits for something?
          // publish is async.

          let broker = broker.clone();
          tokio::spawn(async move {
            // Publish pong
            // Note: publish expects a type that implements MessageSend.
            // We'll publish a String.
            let _ = broker.publish("pong", "pong".to_string()).await;
          });
        }
      }
    }
    Ok(())
  }
}

#[tokio::test]
async fn test_plugin_broadcast() {
  let addr = "127.0.0.1:5006";
  let broker = Broker::start(format!("server {}", addr))
    .await
    .expect("Failed to start broker");

  broker.add_plugin(Box::new(EchoPlugin::new())).await;

  let received_pong = Arc::new(Mutex::new(false));
  let received_pong_clone = received_pong.clone();

  broker.subscribe("pong", move |msg: String| {
    let received_pong = received_pong_clone.clone();
    Box::pin(async move {
      if msg == "pong" {
        let mut lock = received_pong.lock().await;
        *lock = true;
      }
    })
  });

  // Publish ping
  broker
    .publish("ping", "ping".to_string())
    .await
    .expect("Failed to publish ping");

  // Wait for echo
  tokio::time::sleep(Duration::from_secs(1)).await;

  let lock = received_pong.lock().await;
  assert!(*lock, "Did not receive pong from EchoPlugin");
}
