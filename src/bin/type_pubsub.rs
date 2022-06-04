use futures::{future::join_all, join};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use type_pubsub::{new_echo_client, TypePubSub};

#[derive(Debug, Clone)]
pub struct ComposedType {
    id: usize,
    name: String,
}

#[tokio::main]
async fn main() {
    // Create a TypePubSub
    let pubsub = TypePubSub::default();

    // Setup some simple echo clients for `usize` messages
    let handles_usize: Vec<_> = (1..=3)
        .into_iter()
        .map(|num| (num, Arc::clone(&pubsub)))
        .map(|(num, pubsub_)| async move {
            let stream_usize = pubsub_.write().await.subscribe::<usize>();
            new_echo_client(&format!("UsizeEcho{}", num), stream_usize)
        })
        .collect();

    // Setup echo client for `ComposedType`
    let stream_comp = Arc::clone(&pubsub)
        .write()
        .await
        .subscribe::<ComposedType>();
    let handle_comp = new_echo_client("CompEcho", stream_comp);

    // Setup usize sender
    let pubsub_ = Arc::clone(&pubsub);
    let sender_usize_handle = tokio::spawn(async move {
        for i in 0..10 {
            {
                println!("Sending i: {}", i);
                let pubsub_ = pubsub_.read().await;

                pubsub_.publish(i as usize);

                let comp_type = ComposedType {
                    id: i,
                    name: format!("message{}", i),
                };

                pubsub_.publish(comp_type);
            }

            sleep(Duration::from_secs(1)).await;
        }
    });

    // Setup ComposedType sender
    let pubsub_ = Arc::clone(&pubsub);
    let sender_comp_handle = tokio::spawn(async move {
        for i in 0..6 {
            {
                let pubsub_ = pubsub_.read().await;

                let comp_type = ComposedType {
                    id: i,
                    name: format!("message{}", i),
                };
                println!("Sending: {:?}", &comp_type);

                pubsub_.publish(comp_type);
            }

            sleep(Duration::from_secs(2)).await;
        }
    });

    // We want our echo clients to shut down once our sending tasks are done.
    // To achieve this, we have to drop all sending ends of the channels.
    // We can drop all references to the `TypePubSub`, thus the only living
    // channel ends are in the sending tasks.
    // Note that this could also be done implicitly by moving the main `pubsub`
    // reference into the async block of the last task instead of cloning the
    // Arc before.
    drop(pubsub);

    // Run all tasks in parallel
    let _ = join!(
        sender_usize_handle,
        sender_comp_handle,
        handle_comp,
        join_all(handles_usize)
    );
}
