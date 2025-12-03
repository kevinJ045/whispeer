# Whispeer: Secure & Extensible Pub/Sub in Rust

A lightweight, end-to-end encrypted Pub/Sub broker built for secure, real-time applications and extensible event-driven systems. Whispeer leverages Rust's performance and safety features to provide a robust messaging solution.

## Why Whispeer?

Whispeer is designed for developers who need a reliable, secure, and flexible messaging backbone. Whether you're building real-time chat applications, IoT platforms, or distributed microservices, Whispeer offers:

*   **End-to-End Encryption:** Messages are encrypted from publisher to subscriber, ensuring your data remains private and secure.
*   **Pluggable Architecture:** Easily extend the broker's capabilities with custom plugins. Out-of-the-box, it supports compression, encryption, and WebSocket transport.
*   **Typed & Async API:** Enjoy a natural Rust development experience with type-safe messages and full asynchronous support powered by Tokio.
*   **QUIC-based Transport:** Built on QUIC for efficient, low-latency, and secure network communication.
*   **Lightweight & Performant:** A Rust-native solution focused on minimal overhead and high throughput.

## Quick Start

Get a taste of Whispeer with a simple publish-subscribe example:

```rust
use whispeer::Broker;
use serde::{Serialize, Deserialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
struct MyData {
  name: String,
  id: i32
}

#[tokio::main]
async fn main() {
  let broker = Broker::new();
  let topic = "my-secure-channel";

  // Subscribe to a topic with your custom data type
  broker.subscribe(topic, |data: MyData| {
    Box::pin(async move {
      println!("Received data: {:?}", data);
    })
  });

  // Publish a message
  let my_data = MyData {
    name: "Alice".to_string(),
    id: 42
  };

  if let Err(e) = broker.publish(topic, my_data).await {
    eprintln!("Failed to publish: {}", e);
  }
}
```

## Project Details & Development

For a deeper dive into Whispeer's architecture, development milestones, detailed feature lists, and technical stack, please refer to [PROJECT.md](PROJECT.md).

### Contributing

We welcome contributions! Please see [PROJECT.md](PROJECT.md) for guidelines on how to get involved.

### Running Tests

To ensure everything is working as expected, run the test suite:

```bash
cargo test
```
