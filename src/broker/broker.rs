use crate::broker::subscriber::Subscriber;
use crate::broker::topic::{Topic, TopicOperations};
use crate::plugins::manager::PluginManager;
use crate::plugins::plugin::Plugin;
use crate::transport::quic::{make_client_endpoint, make_server_endpoint};
use dashmap::DashMap;
use quinn::{Connection, Endpoint};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

use super::message::{MessageLocal, MessageSend};

/// Represents errors that can occur within the broker.
#[derive(Error, Debug)]
pub enum BrokerError {
  /// Error returned when a topic is not found.
  #[error("Topic not found")]
  TopicNotFound,
  /// Error returned when there is a type mismatch for a topic.
  #[error("Type mismatch")]
  TypeMismatch,
  /// Error returned for network-related issues.
  #[error("Network error: {0}")]
  NetworkError(String),
  /// Error returned from a plugin.
  #[error("Plugin error: {0}")]
  PluginError(String),
}

/// A message format for sending data over the wire, including topic, payload, and headers.
#[derive(Serialize, Deserialize, Debug)]
struct WireMessage {
  topic: String,
  payload: Vec<u8>,
  headers: HashMap<String, String>,
}

/// The central component of the pub/sub system.
///
/// The `Broker` is responsible for managing topics, subscribers, and network peers.
/// It handles publishing messages to topics and delivering them to subscribers, both locally
/// and across the network.
#[derive(Clone)]
pub struct Broker {
  topics: Arc<DashMap<String, Box<dyn TopicOperations>>>,
  peers: Arc<DashMap<SocketAddr, Connection>>,
  endpoint: Option<Endpoint>,
  plugin_manager: Arc<RwLock<PluginManager>>,
}

impl Broker {
  /// Creates a new, empty `Broker` instance.
  ///
  /// This broker is not yet connected to the network and has no topics or subscribers.
  pub fn new() -> Self {
    Self {
      topics: Arc::new(DashMap::new()),
      peers: Arc::new(DashMap::new()),
      endpoint: None,
      plugin_manager: Arc::new(RwLock::new(PluginManager::new())),
    }
  }

  /// Adds a plugin to the broker.
  ///
  /// Plugins can extend the broker's functionality by hooking into various events,
  /// such as message publishing or new subscriptions.
  ///
  /// # Arguments
  ///
  /// * `plugin` - An instance of a type that implements the `Plugin` trait.
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

  /// Starts the broker's network listener or connects to a peer.
  ///
  /// The `addr_str` determines the mode of operation:
  /// - `"server <ip>:<port>"`: Starts a server that listens for incoming connections.
  /// - `"client <ip>:<port>"`: Connects to a peer at the specified address.
  ///
  /// # Arguments
  ///
  /// * `addr_str` - A string specifying the mode and address.
  pub async fn start(addr_str: impl Into<String>) -> Result<Self, anyhow::Error> {
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

  /// Main loop for accepting incoming QUIC connections.
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

  /// Handles an individual QUIC connection, listening for incoming streams.
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

  /// Processes a single incoming stream, deserializing the message and publishing it locally.
  async fn handle_stream(&self, mut stream: quinn::RecvStream) -> Result<(), anyhow::Error> {
    println!("Handling Stream");
    let buf = stream.read_to_end(1024 * 1024).await?;

    if let Ok(mut msg) = serde_json::from_slice::<WireMessage>(&buf) {
      println!("Received network message for topic: {}", msg.topic);

      self
        .plugin_manager
        .read()
        .await
        .on_message_received(&msg.topic, &mut msg.payload, &msg.headers)
        .await?;

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

  /// Subscribes to a topic with an asynchronous handler.
  ///
  /// When a message of type `T` is published to the specified topic, the `handler`
  /// will be called with the message.
  ///
  /// # Arguments
  ///
  /// * `topic_name` - The name of the topic to subscribe to.
  /// * `handler` - An async function or closure that takes a message of type `T` and
  ///   returns a pinned future.
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
        if let Err(e) = plugin_manager
          .read()
          .await
          .on_subscribe(&topic_name_clone)
          .await
        {
          eprintln!(
            "Plugin on_subscribe error for topic '{}': {}",
            topic_name_clone, e
          );
        }
      });
    } else {
      eprintln!(
        "Error: Topic '{}' exists with a different type.",
        topic_name
      );
    }
  }

  /// Publishes a message to a topic.
  ///
  /// The message is delivered to all local subscribers and sent over the network
  /// to all connected peers.
  ///
  /// # Arguments
  ///
  /// * `topic_name` - The name of the topic to publish to.
  /// * `message` - The message to publish. It must be serializable.
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

  /// Publishes a message to local subscribers only.
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
  /// Returns the QUIC endpoint of the broker, if it is running.
  pub fn endpoint(&self) -> Option<Endpoint> {
    return self.endpoint.clone();
  }

  /// Retrieves a mutable reference to a topic by its name.
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
