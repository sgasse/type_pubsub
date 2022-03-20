use futures::{future::join_all, join};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use type_pubsub::{new_echo_client, TypeBroker};

#[derive(Debug, Clone)]
pub struct ComposedType {
    id: usize,
    name: String,
}

#[tokio::main]
async fn main() {
    // Create a TypeBroker
    let broker = TypeBroker::default();

    // Setup some simple echo clients for `usize` messages
    let (subscribers_usize, handles_usize): (Vec<_>, Vec<_>) = (1..=3)
        .into_iter()
        .map(|num| new_echo_client::<usize>(&format!("usize_client_{}", num)))
        .unzip();

    // Setup an echo client for `ComposedType` messages
    let (tx_comp, handle_comp) = new_echo_client::<ComposedType>("comp_client");

    // Subscribe clients to the broker
    {
        let mut broker_ = broker.write().await;

        for subscriber in subscribers_usize.into_iter() {
            broker_.subscribe(subscriber);
        }
        broker_.subscribe(tx_comp);
    }

    // Setup usize sender
    let broker_ = Arc::clone(&broker);
    let sender_usize_handle = tokio::spawn(async move {
        for i in 0..10 {
            {
                println!("Sending i: {}", i);
                let broker_ = broker_.read().await;

                broker_.publish(i as usize);

                let comp_type = ComposedType {
                    id: i,
                    name: format!("message{}", i),
                };

                broker_.publish(comp_type);
            }

            sleep(Duration::from_secs(1)).await;
        }
    });

    // Setup ComposedType sender
    let broker_ = Arc::clone(&broker);
    let sender_comp_handle = tokio::spawn(async move {
        for i in 0..6 {
            {
                let broker_ = broker_.read().await;

                let comp_type = ComposedType {
                    id: i,
                    name: format!("message{}", i),
                };
                println!("Sending: {:?}", &comp_type);

                broker_.publish(comp_type);
            }

            sleep(Duration::from_secs(2)).await;
        }
    });

    let _ = join!(
        sender_usize_handle,
        sender_comp_handle,
        handle_comp,
        join_all(handles_usize)
    );
}
