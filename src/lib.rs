use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::{
    sync::mpsc::{self, UnboundedSender},
    task::JoinHandle,
};

/// Shorthand type for a collection of subscribers
type Subscribers<T> = Vec<UnboundedSender<T>>;

pub struct TypePubSub {
    /// Mapping from type name to collection of subscribers
    subscribers_map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypePubSub {
    /// Create a new instance.
    pub fn default() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(TypePubSub {
            subscribers_map: HashMap::new(),
        }))
    }

    /// Subscribe to message type `T` by providing a sending channel end.
    pub fn subscribe<T: Send + 'static>(&mut self, tx: UnboundedSender<T>) {
        let type_channels = self
            .subscribers_map
            // Get the subscriber collection for type `T`
            .entry(TypeId::of::<T>())
            // Initialize a new collection if it does not yet exist in the map
            .or_insert_with(|| {
                let empty_subscribers: Subscribers<T> = Vec::new();
                Box::new(empty_subscribers)
            });
        type_channels
            .downcast_mut::<Subscribers<T>>()
            .unwrap()
            .push(tx);
    }

    pub fn publish<T: Send + Clone + 'static>(&self, msg: T) {
        match self.subscribers_map.get(&TypeId::of::<T>()) {
            Some(type_channels) => {
                // We have subscribers for this type - send to all
                for sender in type_channels
                    .downcast_ref::<Subscribers<T>>()
                    .unwrap()
                    .iter()
                {
                    sender
                        .send(msg.clone())
                        // SendError does not implement `Debug`, map the error
                        .map_err(|_err| "Sending failed")
                        .unwrap();
                }
            }
            None => return,
        }
    }
}

/// Create a new client that echos in a debug print what it received.
pub fn new_echo_client<T: Send + core::fmt::Debug + 'static>(
    name: &str,
) -> (UnboundedSender<T>, JoinHandle<()>) {
    let name = name.to_owned();
    let (tx, mut rx) = mpsc::unbounded_channel::<T>();

    let handle = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(msg) => println!("{} received {:?}", name, msg),
                None => {
                    println!("{} shutting down because channel closed", name);
                    break;
                }
            }
        }
    });

    (tx, handle)
}
