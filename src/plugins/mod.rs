//! # Plugins Module
//!
//! This module contains the plugin system for the Whispeer broker. It includes the
//! core `Plugin` trait, a `PluginManager` for managing active plugins, and several
//! built-in plugins such as `CompressionPlugin` and `EncryptionPlugin`.
//!
//! The plugin system allows for extending the broker's functionality with custom logic
//! that can hook into various events, like message publishing and subscriptions.

pub mod compression;
pub mod encryption;
pub mod manager;
pub mod plugin;
pub mod websocket;
