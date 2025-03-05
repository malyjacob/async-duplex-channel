//! # Async Duplex Channel Library
//!
//! This library provides an asynchronous duplex communication channel between multiple clients and a single responder
//! in different asynchronous blocks. It enables multiple clients to communicate with a single responder,
//! allowing a client to send requests and receive responses, while the responder processes requests
//! and sends back responses. It is designed for high-concurrency scenarios and supports both single
//! and batch requests, as well as configurable timeouts and concurrency strategies.
//!
//! ## Key Features
//! - **Asynchronous Communication**: Built on top of `tokio` and `futures`, enabling non-blocking I/O.
//! - **Thread Safety**: Uses `Arc` and `Mutex` to ensure thread-safe communication.
//! - **Flexible Concurrency**: Supports both fixed and dynamic concurrency strategies for request processing.
//! - **Batch Requests**: Allows sending multiple requests in a batch with configurable concurrency.
//! - **Configurable Timeouts**: Provides customizable timeouts for requests and responses.
//! - **Error Handling**: Detailed error types for easy debugging and error recovery.
//!
//! ## Usage
//!
//! ### Basic Example
//!
//! ```rust
//! use async_duplex_channel::{channel, Client, ResponderBuilder, ConcurrencyStrategy};
//! use std::time::Duration;
//! use tokio::runtime::Runtime;
//!
//! fn basic_example() {
//!     let rt = Runtime::new().unwrap();
//!     rt.block_on(async {
//!         // Create a channel with a buffer size of 64 and a default timeout of 2 seconds.
//!         let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
//!
//!         // Build the responder with a simple echo handler.
//!         let responder = responder_builder.build(|req: String| async move {
//!             req
//!         });
//!
//!         // Spawn the responder to process requests.
//!         tokio::spawn(async move {
//!             responder.process_requests().await.unwrap();
//!         });
//!
//!         // Send a request and wait for the response.
//!         let response = client.request("Hello, world!".to_string()).await.unwrap();
//!         println!("Response: {}", response);
//!     });
//! }
//! ```
//!
//! ### Batch Requests Example
//!
//! ```rust
//! use async_duplex_channel::{channel, Client, ResponderBuilder, ConcurrencyStrategy};
//! use std::time::Duration;
//! use tokio::runtime::Runtime;
//!
//! fn batch_requests() {
//!     let rt = Runtime::new().unwrap();
//!     rt.block_on(async {
//!         // Create a channel with a buffer size of 64 and a default timeout of 2 seconds.
//!         let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
//!
//!         // Build the responder with a simple echo handler.
//!         let responder = responder_builder.build(|req: String| async move {
//!             req
//!         });
//!
//!         // Spawn the responder to process requests.
//!         tokio::spawn(async move {
//!             responder.process_requests().await.unwrap();
//!         });
//!
//!         // Send a batch of requests with a concurrency limit of 4.
//!         let requests = vec!["Request 1".to_string(), "Request 2".to_string()];
//!         let responses = client.request_batch(requests, 4).await.unwrap();
//!         for response in responses {
//!             println!("Response: {}", response);
//!         }
//!     });
//! }
//! ```
//!
//! ### Dynamic Concurrency Example
//!
//! ```rust
//! use async_duplex_channel::{channel, Client, ResponderBuilder, ConcurrencyStrategy};
//! use std::time::Duration;
//! use tokio::runtime::Runtime;
//!
//! fn dynamic_concurrency() {
//!     let rt = Runtime::new().unwrap();
//!     rt.block_on(async {
//!         // Create a channel with a buffer size of 64 and a default timeout of 2 seconds.
//!         let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
//!
//!         // Build the responder with a simple echo handler.
//!         let responder = responder_builder.build(|req: String| async move {
//!             req
//!         });
//!
//!         // Spawn the responder to process requests with dynamic concurrency.
//!         tokio::spawn(async move {
//!             responder
//!                 .process_requests_with_strategy(ConcurrencyStrategy::Dynamic(4, 16))
//!                 .await
//!                 .unwrap();
//!         });
//!
//!         // Send a request and wait for the response.
//!         let response = client.request("Hello, world!".to_string()).await.unwrap();
//!         println!("Response: {}", response);
//!     });
//! }
//! ```
//!
//! ## Error Handling
//!
//! The library provides detailed error types through the `Error` enum, which covers common failure
//! scenarios such as send failures, receive failures, timeouts, and internal errors. Each error variant
//! includes a descriptive message for easy debugging.
//!
//! ## Concurrency Strategies
//!
//! The responder supports two concurrency strategies:
//! - **Fixed Concurrency**: Processes a fixed number of requests concurrently.
//! - **Dynamic Concurrency**: Adjusts the concurrency level based on response times, ensuring optimal
//!   resource utilization under varying workloads.
//!
//! ## Performance Considerations
//! - Use `request_batch` for high-throughput scenarios to reduce overhead.
//! - Choose the appropriate concurrency strategy based on your workload characteristics.
//! - Monitor and adjust timeouts to balance responsiveness and resource usage.
//!
//! ## Dependencies
//! - `tokio`: For asynchronous runtime support.
//! - `futures`: For future and stream utilities.
//! - `sharded_slab`: For efficient storage of pending requests.
//! - `thiserror`: For ergonomic error handling.
//!
//! ## License
//! This library is licensed under the MIT License. See the LICENSE file for details.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use futures::channel::{mpsc, oneshot};
use futures::future::join_all;
use futures::prelude::*;

use sharded_slab::Slab;

use tokio::sync::Mutex;
use tokio::time::sleep;

use thiserror::Error;

/// Represents errors that can occur during the operation of the client or responder.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),
    #[error("Response timeout")]
    Timeout,
    #[error("Send response failed: {0}")]
    SendResponseFailed(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// A client for sending requests and receiving responses.
///
/// The client is thread-safe and can be cloned to share across multiple tasks.
/// It supports both single and batch requests, with configurable timeouts.
///
/// # Type Parameters
/// - `Req`: The type of the request, must be `Send + Sync + 'static`.
/// - `Resp`: The type of the response, must be `Send + Sync + 'static`.
#[derive(Clone)]
pub struct Client<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    tx: Arc<Mutex<mpsc::Sender<(usize, Req)>>>,
    pending: Arc<Slab<oneshot::Sender<Resp>>>,
    timeout: Duration,
}

/// A builder for creating a responder.
///
/// The responder processes incoming requests and sends back responses.
/// It supports both fixed and dynamic concurrency strategies.
///
/// # Type Parameters
/// - `Req`: The type of the request, must be `Send + Sync + 'static`.
/// - `Resp`: The type of the response, must be `Send + Sync + 'static`.
pub struct ResponderBuilder<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    req_rx: mpsc::Receiver<(usize, Req)>,
    resp_tx: mpsc::Sender<(usize, Resp)>,
}

impl<Req, Resp> ResponderBuilder<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Builds a responder with the provided request handler.
    ///
    /// # Parameters
    /// - `handler`: A function that processes a request and returns a future resolving to a response.
    ///
    /// # Returns
    /// A `Responder` instance ready to process requests.
    pub fn build<Fut: Future<Output = Resp> + Send, F: (FnMut(Req) -> Fut) + Send + Sync>(
        self,
        handler: F,
    ) -> Responder<Req, Resp, Fut, F> {
        Responder {
            req_rx: self.req_rx,
            resp_tx: self.resp_tx,
            handler,
        }
    }
}

/// A responder that processes incoming requests and sends back responses.
///
/// The responder supports both fixed and dynamic concurrency strategies.
///
/// # Type Parameters
/// - `Req`: The type of the request, must be `Send + Sync + 'static`.
/// - `Resp`: The type of the response, must be `Send + Sync + 'static`.
/// - `Fut`: The future returned by the request handler.
/// - `F`: The type of the request handler function.
pub struct Responder<Req, Resp, Fut, F>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
    Fut: Future<Output = Resp> + Send,
    F: (FnMut(Req) -> Fut) + Send + Sync,
{
    req_rx: mpsc::Receiver<(usize, Req)>,
    resp_tx: mpsc::Sender<(usize, Resp)>,
    handler: F,
}

/// Represents the concurrency strategy for processing requests.
///
/// - `Fixed`: A fixed number of concurrent requests.
/// - `Dynamic`: Dynamically adjusts the concurrency based on response times.
#[derive(Debug, Clone, Copy)]
pub enum ConcurrencyStrategy {
    Fixed(usize),
    Dynamic(usize, usize),
}

impl<Req, Resp> Client<Req, Resp>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Sends a single request with a custom timeout.
    ///
    /// # Parameters
    /// - `req`: The request to send.
    /// - `timeout`: The maximum duration to wait for a response.
    ///
    /// # Returns
    /// - `Ok(Resp)`: The response from the server.
    /// - `Err(Error)`: An error if the request fails or times out.
    pub async fn request_timeout(&mut self, req: Req, timeout: Duration) -> Result<Resp, Error> {
        let (tx, rx) = oneshot::channel();

        let id = match self.pending.insert(tx) {
            Some(id) => id,
            None => {
                return Err(Error::InternalError(
                    "Failed to insert into pending slab".into(),
                ));
            }
        };

        self.tx
            .lock()
            .await
            .send((id, req))
            .map_err(|e| Error::SendFailed(e.to_string()))
            .await?;

        let pending = self.pending.clone();
        tokio::select! {
            resp = rx => {
                resp.map_err(|e| Error::ReceiveFailed(e.to_string()))
            },
            _ = sleep(timeout) => {
                pending.remove(id);
                Err(Error::Timeout)
            }
        }
    }

    /// Sends a single request with the default timeout.
    ///
    /// # Parameters
    /// - `req`: The request to send.
    ///
    /// # Returns
    /// - `Ok(Resp)`: The response from the server.
    /// - `Err(Error)`: An error if the request fails or times out.
    pub async fn request(&mut self, req: Req) -> Result<Resp, Error> {
        self.request_timeout(req, self.timeout).await
    }

    /// Sends a batch of requests with a custom timeout and concurrency limit.
    ///
    /// # Parameters
    /// - `reqs`: An iterator of requests to send.
    /// - `timeout`: The maximum duration to wait for all responses.
    /// - `concurrency`: The maximum number of concurrent requests.
    ///
    /// # Returns
    /// - `Ok(Vec<Resp>)`: A vector of responses from the server.
    /// - `Err(Error)`: An error if any request fails or times out.
    pub async fn request_batch_timeout<ReqSeq>(
        &mut self,
        reqs: ReqSeq,
        timeout: Duration,
        concurrency: usize,
    ) -> Result<Vec<Resp>, Error>
    where
        ReqSeq: IntoIterator<Item = Req> + Send + 'static,
    {
        let req_seq: Vec<_> = reqs.into_iter().collect();
        let count = req_seq.len();

        let mut ids: Vec<usize> = Vec::with_capacity(count);
        let mut rxs: Vec<oneshot::Receiver<Resp>> = Vec::with_capacity(count);

        for _ in 0..count {
            let (tx, rx) = oneshot::channel();
            let id = self.pending.insert(tx).unwrap();

            ids.push(id);
            rxs.push(rx);
        }

        stream::iter(ids.clone().into_iter().zip(req_seq.into_iter()))
            .for_each_concurrent(concurrency, |v| async {
                self.tx.lock().await.send(v).await.unwrap();
            })
            .await;

        tokio::select! {
            results = join_all(rxs) => {
                let results = results
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| Error::ReceiveFailed(e.to_string()))?;
                Ok(results)
            },
            _ = sleep(timeout) => {
                for &id in ids.iter() {
                    self.pending.remove(id);
                }
                Err(Error::Timeout)
            }
        }
    }

    /// Sends a batch of requests with the default timeout and a concurrency limit.
    ///
    /// # Parameters
    /// - `reqs`: An iterator of requests to send.
    /// - `concurrency`: The maximum number of concurrent requests.
    ///
    /// # Returns
    /// - `Ok(Vec<Resp>)`: A vector of responses from the server.
    /// - `Err(Error)`: An error if any request fails or times out.
    pub async fn request_batch<ReqSeq>(
        &mut self,
        reqs: ReqSeq,
        concurrency: usize,
    ) -> Result<Vec<Resp>, Error>
    where
        ReqSeq: IntoIterator<Item = Req> + Send + 'static,
    {
        self.request_batch_timeout(reqs, self.timeout * (concurrency as u32), concurrency)
            .await
    }
}

impl<Req, Resp, Fut, F> Responder<Req, Resp, Fut, F>
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
    Fut: Future<Output = Resp> + Send,
    F: (FnMut(Req) -> Fut) + Send + Sync,
{
    /// Processes incoming requests using the specified concurrency strategy.
    ///
    /// # Parameters
    /// - `strategy`: The concurrency strategy to use (`Fixed` or `Dynamic`).
    ///
    /// # Returns
    /// - `Ok(())`: If all requests are processed successfully.
    /// - `Err(Error)`: If an error occurs during processing.
    pub async fn process_requests_with_strategy(
        self,
        strategy: ConcurrencyStrategy,
    ) -> Result<(), Error> {
        match strategy {
            ConcurrencyStrategy::Fixed(concurrency) => {
                self.process_requests_fixed(concurrency).await
            }
            ConcurrencyStrategy::Dynamic(initial, max) => {
                self.process_requests_dynamic(initial, max).await
            }
        }
    }

    /// Processes incoming requests with a fixed concurrency of 16.
    ///
    /// # Returns
    /// - `Ok(())`: If all requests are processed successfully.
    /// - `Err(Error)`: If an error occurs during processing.
    pub async fn process_requests(self) -> Result<(), Error> {
        self.process_requests_fixed(16).await
    }

    /// Processes incoming requests with a fixed concurrency.
    ///
    /// # Parameters
    /// - `concurrency`: The number of concurrent requests to process.
    ///
    /// # Returns
    /// - `Ok(())`: If all requests are processed successfully.
    /// - `Err(Error)`: If an error occurs during processing.
    pub async fn process_requests_fixed(self, concurrency: usize) -> Result<(), Error> {
        let Self {
            req_rx,
            resp_tx,
            mut handler,
        } = self;
        req_rx
            .map(move |(id, req)| {
                let handler = Box::pin(handler(req));
                let fut = handler;
                fut.map(move |resp| Ok((id, resp)))
            })
            .buffer_unordered(concurrency)
            .forward(resp_tx)
            .map_err(|e| Error::SendResponseFailed(e.to_string()))
            .await?;

        Ok(())
    }

    /// Processes incoming requests with dynamic concurrency adjustment.
    ///
    /// The concurrency is adjusted based on the response time of requests.
    ///
    /// # Parameters
    /// - `initial_concurrency`: The initial number of concurrent requests.
    /// - `max_concurrency`: The maximum number of concurrent requests.
    ///
    /// # Returns
    /// - `Ok(())`: If all requests are processed successfully.
    /// - `Err(Error)`: If an error occurs during processing.
    pub async fn process_requests_dynamic(
        self,
        initial_concurrency: usize,
        max_concurrency: usize,
    ) -> Result<(), Error> {
        let Self {
            req_rx,
            resp_tx,
            mut handler,
        } = self;

        let concurrency = Arc::new(AtomicUsize::new(initial_concurrency));

        let concurrency_cloned = concurrency.clone();
        req_rx
            .map(move |(id, req)| {
                let concurrency = Arc::clone(&concurrency);
                let start = Instant::now();

                let fut = Box::pin(handler(req));

                fut.map(move |resp| {
                    let dur = start.elapsed();
                    let currency = concurrency.load(Ordering::Relaxed);
                    if dur < Duration::from_millis(10) {
                        if currency < max_concurrency {
                            let increment = std::cmp::max(1, currency / 4);
                            let new_value =
                                std::cmp::min(max_concurrency, currency.saturating_add(increment));
                            concurrency.store(new_value, Ordering::Relaxed);
                        }
                    } else if dur < Duration::from_millis(100) {
                        if currency > 1 {
                            concurrency.fetch_sub(1, Ordering::Relaxed);
                        }
                    } else {
                        let decrement = std::cmp::max(2, currency / 4);
                        let new_value = std::cmp::min(1, currency.saturating_sub(decrement));
                        concurrency.store(new_value, Ordering::Relaxed);
                    }
                    Ok((id, resp))
                })
            })
            .buffer_unordered(concurrency_cloned.load(Ordering::Relaxed))
            .forward(resp_tx)
            .map_err(|e| Error::SendResponseFailed(e.to_string()))
            .await?;

        Ok(())
    }
}

/// Creates a new client-responder pair with the specified buffer size and timeout.
///
/// # Parameters
/// - `buffer`: The size of the request and response buffers.
/// - `timeout`: The default timeout for requests.
///
/// # Returns
/// A tuple containing a `Client` and a `ResponderBuilder`.
pub fn channel<Req, Resp>(
    buffer: usize,
    timeout: Duration,
) -> (Client<Req, Resp>, ResponderBuilder<Req, Resp>)
where
    Req: Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    let (req_tx, req_rx) = mpsc::channel(buffer);
    let (resp_tx, resp_rx) = mpsc::channel(buffer);

    let client = Client {
        tx: Arc::new(Mutex::new(req_tx)),
        pending: Arc::new(Slab::new()),
        timeout,
    };

    let responder_builder = ResponderBuilder { req_rx, resp_tx };

    let pending = client.pending.clone();

    tokio::spawn(async move {
        resp_rx
            .for_each_concurrent(64, |(id, res)| {
                let pending = pending.clone();
                async move {
                    if let Some(tx) = pending.take(id) {
                        let _ = tx.send(res);
                    }
                }
            })
            .await;
    });

    (client, responder_builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::runtime::Runtime;

    /// Test sending a single request and receiving a response.
    #[test]
    fn test_single_request() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move { req });

            tokio::spawn(async move {
                responder.process_requests().await.unwrap();
            });

            let response = client.request("Hello, world!".to_string()).await.unwrap();
            assert_eq!(response, "Hello, world!");
        });
    }

    /// Test sending a batch of requests and receiving responses.
    #[test]
    fn test_batch_requests() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move { req });

            tokio::spawn(async move {
                responder.process_requests().await.unwrap();
            });

            let requests = vec!["Request 1".to_string(), "Request 2".to_string()];
            let responses = client.request_batch(requests, 4).await.unwrap();
            assert_eq!(
                responses,
                vec!["Request 1".to_string(), "Request 2".to_string()]
            );
        });
    }

    /// Test processing requests with fixed concurrency.
    #[test]
    fn test_fixed_concurrency() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move { req });

            tokio::spawn(async move {
                responder
                    .process_requests_with_strategy(ConcurrencyStrategy::Fixed(16))
                    .await
                    .unwrap();
            });

            let response = client.request("Hello, world!".to_string()).await.unwrap();
            assert_eq!(response, "Hello, world!");
        });
    }

    /// Test processing requests with dynamic concurrency.
    #[test]
    fn test_dynamic_concurrency() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move { req });

            tokio::spawn(async move {
                responder
                    .process_requests_with_strategy(ConcurrencyStrategy::Dynamic(4, 16))
                    .await
                    .unwrap();
            });

            let response = client.request("Hello, world!".to_string()).await.unwrap();
            assert_eq!(response, "Hello, world!");
        });
    }

    /// Test sending a single request with a custom timeout.
    #[test]
    fn test_single_request_timeout() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move { req });

            tokio::spawn(async move {
                responder.process_requests().await.unwrap();
            });

            let response = client
                .request_timeout("Hello, world!".to_string(), Duration::from_secs(1))
                .await
                .unwrap();
            assert_eq!(response, "Hello, world!");
        });
    }

    /// Test sending a batch of requests with a custom timeout.
    #[test]
    fn test_batch_requests_timeout() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move { req });

            tokio::spawn(async move {
                responder.process_requests().await.unwrap();
            });

            let requests = vec!["Request 1".to_string(), "Request 2".to_string()];
            let responses = client
                .request_batch_timeout(requests, Duration::from_secs(1), 4)
                .await
                .unwrap();
            assert_eq!(
                responses,
                vec!["Request 1".to_string(), "Request 2".to_string()]
            );
        });
    }

    /// Test error handling for a failed request.
    #[test]
    fn test_request_failure() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move {
                if req == "fail" {
                    Err(Error::InternalError("Request failed".into()))
                } else {
                    Ok(req)
                }
            });

            tokio::spawn(async move {
                responder.process_requests().await.unwrap();
            });

            let result = client.request("fail".to_string()).await.unwrap();
            assert!(matches!(result, Err(Error::InternalError(_))));
        });
    }

    /// Test error handling for a timeout.
    #[test]
    fn test_request_timeout() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut client, responder_builder) = channel(64, Duration::from_secs(2));
            let responder = responder_builder.build(|req: String| async move {
                tokio::time::sleep(Duration::from_secs(3)).await;
                req
            });

            tokio::spawn(async move {
                responder.process_requests().await.unwrap();
            });

            let result = client
                .request_timeout("Hello, world!".to_string(), Duration::from_secs(1))
                .await;
            assert!(matches!(result, Err(Error::Timeout)));
        });
    }
}
