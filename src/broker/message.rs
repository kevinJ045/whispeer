use serde::{Deserialize, Serialize};

/// A trait for messages that can be passed within a single broker instance.
///
/// This trait requires that the message is `Send`, `Sync`, and `Clone`.
pub trait MessageLocal<'a>: 'a + Send + Sync + Clone {}
impl<'a, T: 'a + Send + Sync + Clone> MessageLocal<'a> for T {}

/// A trait for messages that can be sent over the network.
///
/// This trait builds on `MessageLocal` and adds the `Serialize` requirement,
/// allowing the message to be serialized for transmission.
pub trait MessageSend<'a>: MessageLocal<'a> + Serialize {}
impl<'a, T: MessageLocal<'a> + Serialize> MessageSend<'a> for T {}

/// A trait for messages that can be received from the network.
///
/// This trait builds on `MessageLocal` and adds the `Deserialize` requirement,
/// allowing the message to be deserialized upon receipt.
pub trait MessageRecv<'a>: MessageLocal<'a> + Deserialize<'a> {}
impl<'a, T: MessageLocal<'a> + Deserialize<'a>> MessageRecv<'a> for T {}

/// A comprehensive trait for messages that can be both sent and received.
///
/// This trait combines `MessageSend` and `MessageRecv`, making it suitable for
/// full duplex communication.
pub trait Message<'a>: MessageSend<'a> + MessageRecv<'a> {}
impl<'a, T: MessageRecv<'a> + MessageSend<'a>> Message<'a> for T {}
