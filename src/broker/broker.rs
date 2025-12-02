use crate::broker::subscriber::Subscriber;
use crate::broker::topic::{Topic, TopicOperations};
use crate::transport::quic::{make_client_endpoint, make_server_endpoint};
use dashmap::DashMap;
use quinn::{Connection, Endpoint};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;

use super::message::{MessageLocal, MessageSend};

#[derive(Error, Debug)]
pub enum BrokerError {
  #[error("Topic not found")]
  TopicNotFound,
  #[error("Type mismatch")]
  TypeMismatch,
  #[error("Network error: {0}")]
  NetworkError(String),
}

#[derive(Serialize, Deserialize, Debug)]
struct WireMessage {
  topic: String,
  payload: String, // For MVP, we treat everything as String over the wire
}

/// The Broker manages topics and subscribers.
#[derive(Clone)]
pub struct Broker {
  topics: Arc<DashMap<String, Box<dyn TopicOperations>>>,
  peers: Arc<DashMap<SocketAddr, Connection>>,
  endpoint: Option<Endpoint>,
}

impl Broker {
  pub fn new() -> Self {
    Self {
      topics: Arc::new(DashMap::new()),
      peers: Arc::new(DashMap::new()),
      endpoint: None,
    }
  }

  /// Starts the broker on the given address.
  /// Tries to bind (listen). If it fails, tries to connect to the address.
  pub async fn start(addr_str: impl Into<String>) -> Result<Self, Box<dyn std::error::Error>> {
    let addr_str = addr_str.into();
    let addr_string: Vec<&str> = addr_str.split(" ").collect();
    let mode = addr_string[0].to_string();
    let addr: SocketAddr = addr_string[1].parse()?;

    if mode == "server" {
      let (endpoint, _cert) = make_server_endpoint(addr)?;
      println!("Broker listening on {}", addr);
      let broker = Self {
        topics: Arc::new(DashMap::new()),
        peers: Arc::new(DashMap::new()),
        endpoint: Some(endpoint.clone()),
      };

      // Spawn listener task
      let broker_clone = broker.clone();
      tokio::spawn(async move {
        broker_clone.accept_loop(endpoint).await;
      });

      Ok(broker)
    } else {
      println!("Address {} in use, trying to connect...", addr);
      // Try to connect as client
      // We need to bind to a different port (0)
      let client_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
      // We need the server cert. For this MVP, we are skipping verification in make_client_endpoint
      // so we can pass a dummy cert or handle it inside.
      // make_client_endpoint takes server_cert.
      // In a real scenario we'd need the cert.
      // Let's modify make_client_endpoint to accept empty if we are skipping.
      let endpoint = make_client_endpoint(client_addr, &[])?;

      let connecting = endpoint.connect(addr, "localhost")?;
      let connection = connecting.await?;

      println!("Connected to peer at {}", addr);

      let broker = Self {
        topics: Arc::new(DashMap::new()),
        peers: Arc::new(DashMap::new()),
        endpoint: Some(endpoint),
      };

      broker.peers.insert(addr, connection.clone());

      // Spawn listener for this connection (bidirectional)
      let broker_clone = broker.clone();
      tokio::spawn(async move {
        broker_clone.handle_connection(connection).await;
      });

      Ok(broker)
    }
  }

  pub async fn accept_loop(&self, endpoint: Endpoint) {
    println!("Accept loop running");

    while let Some(connecting) = endpoint.accept().await {
      let self_clone = self.clone();

      tokio::spawn(async move {
        match connecting.await {
          Ok(conn) => {
            println!("New connection");
            self_clone.handle_connection(conn).await;
          }
          Err(e) => eprintln!("Connection failed: {e}"),
        }
      });
    }
  }

  async fn handle_connection(&self, connection: quinn::Connection) {
    println!("Handling connection");

    loop {
      match connection.accept_uni().await {
        Ok(stream) => {
          println!("Received something");
          let self_clone = self.clone();
          tokio::spawn(async move {
            if let Err(e) = self_clone.handle_stream(stream).await {
              eprintln!("Error handling stream: {}", e);
            }
          });
        }
        Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
          println!("Connection closed");
          break;
        }
        Err(e) => {
          eprintln!("accept_uni error: {:?}", e);
          break;
        }
      }
    }
  }

  async fn handle_stream(
    &self,
    mut stream: quinn::RecvStream,
  ) -> Result<(), Box<dyn std::error::Error>> {
    println!("Handling Stream");
    let buf = stream.read_to_end(1024 * 1024).await?;

    if let Ok(msg) = serde_json::from_slice::<WireMessage>(&buf) {
      println!("Received network message for topic: {}", msg.topic);
      if let Some(topic) = self.topics.get(&msg.topic) {
        topic.publish_json(&msg.payload).await;
      }
    }

    Ok(())
  }

  /// Subscribes to a topic. Creates the topic if it doesn't exist.
  pub fn subscribe<T: 'static + Send + Sync + Clone + DeserializeOwned>(
    &self,
    topic_name: impl Into<String>,
    handler: impl Fn(T) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    + Send
    + Sync
    + 'static,
  ) {
    let topic_name = topic_name.into();
    let mut entry = self
      .topics
      .entry(topic_name.clone())
      .or_insert_with(|| Box::new(Topic::<T>::new(topic_name.clone())));

    if let Some(topic) = entry.as_any_mut().downcast_mut::<Topic<T>>() {
      let subscriber = Subscriber::new(handler);
      topic.add_subscriber(subscriber);
    } else {
      eprintln!(
        "Error: Topic '{}' exists with a different type.",
        topic_name
      );
    }
  }

  /// Publishes a message to a topic (local and remote).
  pub async fn publish<T: MessageSend<'static>>(
    &self,
    topic_name: impl Into<String>,
    message: T,
  ) -> Result<(), BrokerError> {
    // 1. Publish locally
    let topic_name = topic_name.into();
    self.publish_local(&topic_name, message.clone()).await?;

    // 2. Publish to peers
    // We serialize to String for MVP wire format
    let payload =
      serde_json::to_string(&message).map_err(|e| BrokerError::NetworkError(e.to_string()))?;
    let wire_msg = WireMessage {
      topic: topic_name,
      payload,
    };
    let bytes =
      serde_json::to_vec(&wire_msg).map_err(|e| BrokerError::NetworkError(e.to_string()))?;

    for peer in self.peers.iter() {
      let connection = peer.value();
      match connection.open_uni().await {
        Ok(mut stream) => {
          if let Err(e) = stream.write_all(&bytes).await {
            eprintln!("Failed to send to peer: {}", e);
          }
          let _ = stream.finish().await;
        }
        Err(e) => eprintln!("Failed to open stream to peer: {}", e),
      }
    }

    Ok(())
  }

  // Internal helper for local publish
  async fn publish_local<T: MessageLocal<'static>>(
    &self,
    topic_name: &String,
    message: T,
  ) -> Result<(), BrokerError> {
    if let Some(mut entry) = self.topics.get_mut(topic_name) {
      if let Some(topic) = entry.as_any_mut().downcast_mut::<Topic<T>>() {
        for sub in &topic.subscribers {
          let msg_clone = message.clone();
          let handler = &sub.handler;
          (handler)(msg_clone).await;
        }
        Ok(())
      } else {
        Err(BrokerError::TypeMismatch)
      }
    } else {
      Ok(())
    }
  }
}

impl Broker {
  pub fn endpoint(&self) -> Option<Endpoint> {
    return self.endpoint.clone();
  }
}

impl Default for Broker {
  fn default() -> Self {
    Self::new()
  }
}
