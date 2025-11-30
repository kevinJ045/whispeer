pub mod broker;
pub mod transport;

pub use broker::Broker;

#[cfg(test)]
mod tests {
  use super::*;
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
    let addr_a = "127.0.0.1:5001";
    let broker_a = Broker::start(format!("server {addr_a}"))
      .await
      .expect("Failed to start broker A");

    // Broker B - Connector
    // We give it the same address so it tries to bind, fails, and connects to A
    let broker_b = Broker::start(format!("client {addr_a}"))
      .await
      .expect("Failed to start broker B");

    // Subscribe on A
    let received_msg = Arc::new(Mutex::new(None));
    let received_msg_clone = received_msg.clone();
    broker_a.subscribe("chat", move |msg: String| {
      println!("{msg}");
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
    tokio::time::sleep(Duration::from_secs(10)).await;

    let lock = received_msg.lock().await;
    assert_eq!(*lock, Some(msg));
  }
}
