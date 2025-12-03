use crate::broker::subscriber::Subscriber;
use crate::broker::topic::{Topic, TopicOperations};
use crate::plugins::manager::PluginManager;
use crate::plugins::plugin::Plugin;
use crate::transport::quic::{make_client_endpoint, make_server_endpoint};
use dashmap::DashMap;
use quinn::{Connection, Endpoint};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

use super::message::{MessageLocal, MessageSend};

#[derive(Error, Debug)]
pub enum BrokerError {
  #[error("Topic not found")]
  TopicNotFound,
  #[error("Type mismatch")]
  TypeMismatch,
  #[error("Network error: {0}")]
  NetworkError(String),
  #[error("Plugin error: {0}")]
  PluginError(String),
}

#[derive(Serialize, Deserialize, Debug)]
struct WireMessage {
  topic: String,
  payload: Vec<u8>,
  headers: HashMap<String, String>,
}

#[derive(Clone)]
pub struct Broker {
  topics: Arc<DashMap<String, Box<dyn TopicOperations>>>,
  peers: Arc<DashMap<SocketAddr, Connection>>,
  endpoint: Option<Endpoint>,
  plugin_manager: Arc<RwLock<PluginManager>>,
}

impl Broker {
  pub fn new() -> Self {
    Self {
      topics: Arc::new(DashMap::new()),
      peers: Arc::new(DashMap::new()),
      endpoint: None,
      plugin_manager: Arc::new(RwLock::new(PluginManager::new())),
    }
  }

  pub async fn add_plugin(&self, mut plugin: impl Plugin + 'static) {
    if let Err(e) = plugin.on_init(self).await {
      eprintln!("Plugin {} failed to initialize: {}", plugin.name(), e);
      return;
    }
    self
      .plugin_manager
      .write()
      .await
      .add_plugin(Box::new(plugin));
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
        plugin_manager: Arc::new(RwLock::new(PluginManager::new())),
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
        plugin_manager: Arc::new(RwLock::new(PluginManager::new())),
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

  async fn handle_stream(&self, mut stream: quinn::RecvStream) -> Result<(), anyhow::Error> {
    println!("Handling Stream");
    let buf = stream.read_to_end(1024 * 1024).await?;

    if let Ok(mut msg) = serde_json::from_slice::<WireMessage>(&buf) {
      println!("Received network message for topic: {}", msg.topic);

      // Run plugins on received message
      self
        .plugin_manager
        .read()
        .await
        .on_message_received(&msg.topic, &mut msg.payload, &msg.headers)
        .await?;

      // Convert payload back to string for MVP JSON publishing
      // This assumes the payload is valid UTF-8 JSON after plugin processing (decompression)
      if let Ok(payload_str) = String::from_utf8(msg.payload) {
        if let Some(topic) = self.topics.get(&msg.topic) {
          topic.publish_json(&payload_str).await;
        }
      } else {
        eprintln!("Failed to convert payload to string after processing");
      }
    }

    Ok(())
  }

  /// Subscribes to a topic. Creates the topic if it doesn't exist.
  pub fn subscribe<T: 'static + Send + Sync + Clone + DeserializeOwned + Serialize>(
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

      // Call on_subscribe hook for plugins
      let plugin_manager = self.plugin_manager.clone();
      let topic_name_clone = topic_name.clone();
      tokio::spawn(async move {
        if let Err(e) = plugin_manager.read().await.on_subscribe(&topic_name_clone).await {
          eprintln!("Plugin on_subscribe error for topic '{}': {}", topic_name_clone, e);
        }
      });
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
    // Serialize message to JSON string first
    let payload_str =
      serde_json::to_string(&message).map_err(|e| BrokerError::NetworkError(e.to_string()))?;
    let mut payload = payload_str.into_bytes();
    let mut headers = HashMap::new();

    // Run plugins on publish
    self
      .plugin_manager
      .read()
      .await
      .on_publish(&topic_name, &mut payload, &mut headers)
      .await
      .map_err(|e| BrokerError::PluginError(e.to_string()))?;

    let wire_msg = WireMessage {
      topic: topic_name,
      payload,
      headers,
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

  pub fn get_topic(
    &self,
    name: &str,
  ) -> Option<dashmap::mapref::one::RefMut<String, Box<dyn TopicOperations>>> {
    self.topics.get_mut(name)
  }
}

impl Default for Broker {
  fn default() -> Self {
    Self::new()
  }
}
