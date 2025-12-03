use crate::plugins::plugin::Plugin;
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};

/// A plugin that exposes the broker's functionality over a WebSocket connection.
///
/// This plugin starts a WebSocket server that clients can connect to in order to
/// subscribe to topics and publish messages.
pub struct WebSocketPlugin {
  addr: String,
}

impl WebSocketPlugin {
  /// Creates a new `WebSocketPlugin` that will listen on the given address.
  ///
  /// # Arguments
  ///
  /// * `addr` - The address to bind the WebSocket server to (e.g., "127.0.0.1:8080").
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

  /// Initializes the plugin by starting the WebSocket server in a new task.
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

/// Handles a single WebSocket connection.
///
/// This function manages the communication with a single client, parsing incoming
/// messages and handling the text-based protocol for subscribing and publishing.
///
/// The protocol is as follows:
/// - `SUBSCRIBE <topic>`: Subscribes the client to the specified topic.
/// - `PUBLISH <topic> <json_payload>`: Publishes a message to the specified topic.
async fn handle_connection(stream: TcpStream, broker: crate::Broker) -> Result<(), anyhow::Error> {
  let ws_stream = accept_async(stream).await.expect("Failed to accept");
  let (mut ws_sender, mut ws_receiver) = ws_stream.split();

  // Create a channel to send messages to the WS client from other tasks (subscriptions)
  let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

  // Task to forward messages from the channel to the WS client
  let send_task = tokio::spawn(async move {
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
        continue;
      }

      let command = parts[0];
      let rest = parts[1];

      if command == "SUBSCRIBE" {
        let topic_name = rest.trim();
        println!("WS Client subscribing to: {}", topic_name);

        if let Some(mut topic) = broker.get_topic(topic_name) {
          let tx = tx.clone();
          topic.subscribe_json(Box::new(move |payload| {
            let tx = tx.clone();
            Box::pin(async move {
              let _ = tx.send(Message::Text(Utf8Bytes::from(payload)));
            })
          }));
        } else {
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
