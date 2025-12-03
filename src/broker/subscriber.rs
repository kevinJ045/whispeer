use uuid::Uuid;

use std::future::Future;
use std::pin::Pin;

/// A unique identifier for a subscriber.
pub type SubscriberId = Uuid;

/// A type alias for an asynchronous message handler.
///
/// This uses a boxed, pinned future (`Pin<Box<dyn Future>>`) to allow for async closures
/// to be used as message handlers.
pub type AsyncHandler<T> = Box<dyn Fn(T) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Represents a client subscribed to a topic.
///
/// Each subscriber has a unique `id` and an `handler` for processing incoming messages
/// asynchronously.
pub struct Subscriber<T> {
  /// The unique identifier for the subscriber.
  pub id: SubscriberId,
  /// The asynchronous handler for processing messages.
  pub handler: AsyncHandler<T>,
}

impl<T> Subscriber<T> {
  /// Creates a new `Subscriber` with a given handler.
  ///
  /// The handler is a closure or function that takes a message of type `T` and returns a future.
  ///
  /// # Arguments
  ///
  /// * `handler` - The async closure to be executed when a message is received.
  pub fn new<F, Fut>(handler: F) -> Self
  where
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
  {
    Self {
      id: Uuid::new_v4(),
      handler: Box::new(move |msg| Box::pin(handler(msg))),
    }
  }
}

impl<T> std::fmt::Debug for Subscriber<T> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Subscriber").field("id", &self.id).finish()
  }
}
