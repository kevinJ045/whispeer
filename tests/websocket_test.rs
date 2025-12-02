use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{Message, Utf8Bytes};
use whispeer::broker::broker::Broker;
use whispeer::plugins::websocket::WebSocketPlugin;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestMessage {
  content: String,
}

#[tokio::test]
async fn test_websocket_plugin() {
  // 1. Start Broker with WebSocketPlugin
  let broker = Broker::new();
  let ws_plugin = WebSocketPlugin::new("127.0.0.1:9090");
  broker.add_plugin(Box::new(ws_plugin)).await;

  // Give it a moment to start listening
  tokio::time::sleep(Duration::from_millis(100)).await;

  // 2. Connect WS Client
  let (mut ws_stream, _) = connect_async("ws://127.0.0.1:9090")
    .await
    .expect("Failed to connect");

  // 3. Subscribe to "test-topic"
  ws_stream
    .send(Message::Text(Utf8Bytes::from(
      "SUBSCRIBE test-topic".to_string(),
    )))
    .await
    .expect("Failed to subscribe");

  // Give it a moment to register subscription
  tokio::time::sleep(Duration::from_millis(100)).await;

  // 4. Publish from Broker (Rust side)
  let msg = TestMessage {
    content: "Hello from Rust".to_string(),
  };
  // We need to ensure the topic exists as the correct type for the Rust publish to work if we want to use typed publish.
  // However, the WS subscription might have created it as serde_json::Value.
  // If we publish as serde_json::Value it should work.
  let json_msg = serde_json::to_value(&msg).unwrap();
  broker
    .publish("test-topic", json_msg)
    .await
    .expect("Failed to publish");

  // 5. Verify WS Client receives message
  if let Some(Ok(Message::Text(text))) = ws_stream.next().await {
    println!("Received: {}", text);
    let received: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(received["content"], "Hello from Rust");
  } else {
    panic!("Did not receive message");
  }

  // 6. Publish from WS Client
  let ws_msg = serde_json::json!({ "content": "Hello from WS" }).to_string();
  ws_stream
    .send(Message::Text(Utf8Bytes::from(format!(
      "PUBLISH test-topic {}",
      ws_msg
    ))))
    .await
    .expect("Failed to publish from WS");

  // 7. Verify Rust subscriber receives it
  // We need a channel to signal success
  let (tx, mut rx) = tokio::sync::mpsc::channel(1);

  broker.subscribe::<serde_json::Value>("test-topic", move |msg| {
    let tx = tx.clone();
    Box::pin(async move {
      if msg["content"] == "Hello from WS" {
        tx.send(()).await.unwrap();
      }
    })
  });

  // Wait for the message (timeout after 1s)
  let result = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
  assert!(result.is_ok(), "Timed out waiting for message from WS");
}
