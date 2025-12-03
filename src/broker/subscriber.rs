use uuid::Uuid;

use std::future::Future;
use std::pin::Pin;

pub type SubscriberId = Uuid;

/// A message handler that can be async.
/// We use a Boxed Future to allow for async closures.
pub type AsyncHandler<T> = Box<dyn Fn(T) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct Subscriber<T> {
  pub id: SubscriberId,
  pub handler: AsyncHandler<T>,
}

impl<T> Subscriber<T> {
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
