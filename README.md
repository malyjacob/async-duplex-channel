# Async Duplex Channel Library

This library provides an asynchronous duplex communication channel between multiple clients and a single responder 
in different asynchronous blocks. It enables multiple clients to communicate with a single responder, 
allowing a client to send requests and receive responses, while the responder processes requests
and sends back responses. It is designed for high-concurrency scenarios and supports both single
and batch requests, as well as configurable timeouts and concurrency strategies.

## Key Features
- **Asynchronous Communication**: Built on top of `tokio` and `futures`, enabling non-blocking I/O.
- **Thread Safety**: Uses `Arc` and `Mutex` to ensure thread-safe communication.
- **Flexible Concurrency**: Supports both fixed and dynamic concurrency strategies for request processing.
- **Batch Requests**: Allows sending multiple requests in a batch with configurable concurrency.
- **Configurable Timeouts**: Provides customizable timeouts for requests and responses.
- **Error Handling**: Detailed error types for easy debugging and error recovery.

## Usage

### Basic Example

```rust
use async_duplex_channel::{channel, Client, ResponderBuilder, ConcurrencyStrategy};
use std::time::Duration;
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        // Create a channel with a buffer size of 64 and a default timeout of 2 seconds.
        let (mut client, responder_builder) = channel(64, Duration::from_secs(2));

        // Build the responder with a simple echo handler.
        let responder = responder_builder.build(|req: String| async move {
            req
        });

        // Spawn the responder to process requests.
        tokio::spawn(async move {
            responder.process_requests().await.unwrap();
        });

        // Send a request and wait for the response.
        let response = client.request("Hello, world!".to_string()).await.unwrap();
        println!("Response: {}", response);
    });
}
```

### Batch Requests Example

```rust
use async_duplex_channel::{channel, Client, ResponderBuilder, ConcurrencyStrategy};
use std::time::Duration;
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        // Create a channel with a buffer size of 64 and a default timeout of 2 seconds.
        let (mut client, responder_builder) = channel(64, Duration::from_secs(2));

        // Build the responder with a simple echo handler.
        let responder = responder_builder.build(|req: String| async move {
            req
        });

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
```

### Dynamic Concurrency Example

```rust
use async_duplex_channel::{channel, Client, ResponderBuilder, ConcurrencyStrategy};
use std::time::Duration;
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        // Create a channel with a buffer size of 64 and a default timeout of 2 seconds.
        let (mut client, responder_builder) = channel(64, Duration::from_secs(2));

        // Build the responder with a simple echo handler.
        let responder = responder_builder.build(|req: String| async move {
            req
        });

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
```

## Error Handling

The library provides detailed error types through the `Error` enum, which covers common failure
scenarios such as send failures, receive failures, timeouts, and internal errors. Each error variant
includes a descriptive message for easy debugging.

## Concurrency Strategies

The responder supports two concurrency strategies:
- **Fixed Concurrency**: Processes a fixed number of requests concurrently.
- **Dynamic Concurrency**: Adjusts the concurrency level based on response times, ensuring optimal
  resource utilization under varying workloads.

## Performance Considerations
- Use `request_batch` for high-throughput scenarios to reduce overhead.
- Choose the appropriate concurrency strategy based on your workload characteristics.
- Monitor and adjust timeouts to balance responsiveness and resource usage.

## Dependencies
- `tokio`: For asynchronous runtime support.
- `futures`: For future and stream utilities.
- `sharded_slab`: For efficient storage of pending requests.
- `thiserror`: For ergonomic error handling.

## License
This library is licensed under the MIT License. See the LICENSE file for details.
