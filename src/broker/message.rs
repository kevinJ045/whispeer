use serde::{Deserialize, Serialize};

pub trait MessageLocal<'a>: 'a + Send + Sync + Clone {}
impl<'a, T: 'a + Send + Sync + Clone> MessageLocal<'a> for T {}

pub trait MessageSend<'a>: MessageLocal<'a> + Serialize {}
impl<'a, T: MessageLocal<'a> + Serialize> MessageSend<'a> for T {}

pub trait MessageRecv<'a>: MessageLocal<'a> + Deserialize<'a> {}
impl<'a, T: MessageLocal<'a> + Deserialize<'a>> MessageRecv<'a> for T {}

pub trait Message<'a>: MessageSend<'a> + MessageRecv<'a> {}
impl<'a, T: MessageRecv<'a> + MessageSend<'a>> Message<'a> for T {}
