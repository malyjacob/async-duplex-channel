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

        // Spawn the responder to process requests with dynamic concurrency.
        tokio::spawn(async move {
            responder
                .process_requests_with_strategy(ConcurrencyStrategy::Dynamic(4, 16))
                .await
                .unwrap();
        });

        // Send a request and wait for the response.
        let response = client.request("Hello, world!".to_string()).await.unwrap();
        println!("Response: {}", response);
    });
}
