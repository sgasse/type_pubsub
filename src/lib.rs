use futures::{
    channel::mpsc::{self, UnboundedReceiver, UnboundedSender},
    pin_mut, Stream, StreamExt,
};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::{sync::RwLock, task::JoinHandle};

/// Shorthand type for a collection of subscribers
type Subscribers<T> = Vec<UnboundedSender<T>>;

pub struct PubSubStream<T: Sync + Send + Clone + 'static>(UnboundedReceiver<T>);

impl<T: Sync + Send + Clone + 'static> Stream for PubSubStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

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

    /// Subscribe to message type `T`.
    pub fn subscribe<T: Send + Sync + Clone + 'static>(&mut self) -> PubSubStream<T> {
        let (tx, rx) = mpsc::unbounded();
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
        PubSubStream(rx)
    }

    pub fn publish<T: Send + Sync + Clone + 'static>(&self, msg: T) {
        match self.subscribers_map.get(&TypeId::of::<T>()) {
            Some(type_channels) => {
                // We have subscribers for this type - send to all
                for sender in type_channels
                    .downcast_ref::<Subscribers<T>>()
                    .unwrap()
                    .iter()
                {
                    sender
                        .unbounded_send(msg.clone())
                        // SendError does not implement `Debug`, map the error
                        .map_err(|_err| "Sending failed")
                        .unwrap();
                }
            }
            None => return,
        }
    }
}

pub fn new_echo_client<T: Send + Sync + Clone + core::fmt::Debug>(
    name: &str,
    stream: PubSubStream<T>,
) -> JoinHandle<()> {
    let name = name.to_owned();
    tokio::spawn(async move {
        pin_mut!(stream);
        loop {
            match stream.next().await {
                Some(msg) => println!("{} received {:?}", &name, msg),
                None => {
                    println!("{} shutting down because channel closed", name);
                    break;
                }
            }
        }
    })
}
