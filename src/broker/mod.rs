//! # Broker Module
//!
//! This module contains the core components of the Whispeer broker. It includes the main
//! `Broker` struct, `Topic` management, `Subscriber` handling, and the `Message` definitions.
//! This module provides the central publish/subscribe logic for the system.

pub mod broker;
pub mod message;
pub mod subscriber;
pub mod topic;

pub use broker::Broker;
pub use message::{Message, MessageLocal};
pub use subscriber::Subscriber;
pub use topic::Topic;
