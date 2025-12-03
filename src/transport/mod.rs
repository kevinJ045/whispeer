//! # Transport Module
//!
//! This module is responsible for the network transport layer of the broker.
//! It includes implementations for different transport protocols, with the
//! primary implementation being QUIC for secure and efficient communication.

pub mod quic;
