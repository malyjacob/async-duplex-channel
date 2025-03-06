use async_duplex_channel::*;
use std::time::Duration;
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        // Create a channel with a buffer size of 64 and a default timeout of 2 seconds.
        let (client, responder_builder) = channel(64, Duration::from_secs(2));

        // Build the responder with a simple echo handler.
        let responder = responder_builder.build(|req: String| async move { req });

        // Spawn the responder to process requests.
        tokio::spawn(async move {
            responder.process_requests().await.unwrap();
        });

        // Send a batch of requests with a concurrency limit of 4.
        let requests = vec!["Request 1".to_string(), "Request 2".to_string()];
        let responses = client.request_batch(requests, 4).await.unwrap();
        for response in responses {
            println!("Response: {}", response);
        }
    });
}
