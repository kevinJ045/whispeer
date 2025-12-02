use crate::plugins::plugin::Plugin;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};

pub struct WebSocketPlugin {
  addr: String,
}

impl WebSocketPlugin {
  pub fn new(addr: &str) -> Self {
    Self {
      addr: addr.to_string(),
    }
  }
}

#[async_trait]
impl Plugin for WebSocketPlugin {
  fn name(&self) -> &str {
    "WebSocketPlugin"
  }

  async fn on_init(&mut self, broker: &crate::Broker) -> Result<(), anyhow::Error> {
    let addr = self.addr.clone();
    let broker = broker.clone();

    tokio::spawn(async move {
      let try_socket = TcpListener::bind(&addr).await;
      let listener = try_socket.expect("Failed to bind");
      println!("WebSocket server listening on: {}", addr);

      while let Ok((stream, _)) = listener.accept().await {
        let broker = broker.clone();
        tokio::spawn(async move {
          if let Err(e) = handle_connection(stream, broker).await {
            eprintln!("Error processing connection: {}", e);
          }
        });
      }
    });

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
    _topic: &str,
    _payload: &mut Vec<u8>,
    _headers: &HashMap<String, String>,
  ) -> Result<(), anyhow::Error> {
    Ok(())
  }
}

async fn handle_connection(stream: TcpStream, broker: crate::Broker) -> Result<(), anyhow::Error> {
  let ws_stream = accept_async(stream).await.expect("Failed to accept");
  let (mut ws_sender, mut ws_receiver) = ws_stream.split();

  // Create a channel to send messages to the WS client from other tasks (subscriptions)
  let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

  // Task to forward messages from the channel to the WS client
  let mut send_task = tokio::spawn(async move {
    while let Some(message) = rx.recv().await {
      if let Err(e) = ws_sender.send(message).await {
        eprintln!("Error sending message to WS client: {}", e);
        break;
      }
    }
  });

  // Handle incoming messages from WS client
  while let Some(msg) = ws_receiver.next().await {
    let msg = msg?;
    if msg.is_text() || msg.is_binary() {
      let text = msg.to_string();
      // Simple protocol:
      // SUBSCRIBE <topic>
      // PUBLISH <topic> <json_payload>

      let parts: Vec<&str> = text.splitn(2, ' ').collect();
      if parts.len() < 2 {
        // Maybe just payload? Or ignore?
        continue;
      }

      let command = parts[0];
      let rest = parts[1];

      if command == "SUBSCRIBE" {
        let topic_name = rest.trim();
        println!("WS Client subscribing to: {}", topic_name);

        // Ensure topic exists (create if not) - but we need a type.
        // For now, assume generic JSON topic if creating, or attach to existing.
        // Since we don't know the type T, we can't call `subscribe` easily if it creates a new topic.
        // But `subscribe_json` is on `TopicOperations`.
        // We need to ensure the topic exists.
        // Hack: We can't easily create a typed topic without knowing the type.
        // For this MVP, let's assume the topic is created elsewhere or we default to serde_json::Value?
        // Or we can try to get it, and if not found, we can't subscribe yet?
        // Or we create a Topic<serde_json::Value>.

        // Let's try to get the topic.
        if let Some(mut topic) = broker.get_topic(topic_name) {
          let tx = tx.clone();
          topic.subscribe_json(Box::new(move |payload| {
            let tx = tx.clone();
            Box::pin(async move {
              let _ = tx.send(Message::Text(Utf8Bytes::from(payload)));
            })
          }));
        } else {
          // Topic doesn't exist. Create it as Topic<serde_json::Value>
          // We need to call a method on broker to create it.
          // Broker::subscribe creates it.
          // broker.subscribe::<serde_json::Value>(topic_name, ...);
          let tx = tx.clone();
          broker.subscribe::<serde_json::Value>(topic_name, move |payload| {
            let tx = tx.clone();
            Box::pin(async move {
              let json_str = serde_json::to_string(&payload).unwrap_or_default();
              let _ = tx.send(Message::Text(Utf8Bytes::from(json_str)));
            })
          });
        }
      } else if command == "PUBLISH" {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
          let topic_name = parts[0];
          let payload = parts[1];

          // Publish as serde_json::Value to handle generic JSON
          if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Err(e) = broker.publish(topic_name, value).await {
              eprintln!("Failed to publish from WS: {}", e);
            }
          } else {
            eprintln!("Invalid JSON from WS");
          }
        }
      }
    } else if msg.is_close() {
      break;
    }
  }

  send_task.abort();
  Ok(())
}
