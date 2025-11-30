use crate::broker::subscriber::Subscriber;

pub struct Topic<T> {
    pub name: String,
    pub subscribers: Vec<Subscriber<T>>,
}

impl<T> Topic<T> {
    pub fn new(name: String) -> Self {
        Self {
            name,
            subscribers: Vec::new(),
        }
    }

    pub fn add_subscriber(&mut self, subscriber: Subscriber<T>) {
        self.subscribers.push(subscriber);
    }

    pub fn remove_subscriber(&mut self, subscriber_id: uuid::Uuid) {
        self.subscribers.retain(|s| s.id != subscriber_id);
    }
}
