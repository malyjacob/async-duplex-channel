use async_duplex_channel::{ConcurrencyStrategy, channel};
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark for sending a single request.
fn bench_single_request() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async move {
        let (client, responder_builder) = channel(64, Duration::from_secs(2));
        let responder = responder_builder.build(|req: String| async move { req });

        tokio::spawn(async move {
            responder.process_requests().await.unwrap();
        });
        let response = client
            .request(black_box("Hello, world!".to_string()))
            .await
            .unwrap();
        black_box(response);
    });
}

/// Benchmark for sending a batch of requests.
fn bench_batch_requests() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async move {
        let (client, responder_builder) = channel(64, Duration::from_secs(2));
        let responder = responder_builder.build(|req: String| async move { req });

        tokio::spawn(async move {
            responder.process_requests().await.unwrap();
        });

        let requests = vec![
            black_box("Request 1".to_string()),
            black_box("Request 2".to_string()),
        ];
        let responses = client.request_batch(requests, 4).await.unwrap();
        black_box(responses);
    });
}

/// Benchmark for processing requests with fixed concurrency.
fn bench_fixed_concurrency() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async move {
        let (client, responder_builder) = channel(64, Duration::from_secs(2));
        let responder = responder_builder.build(|req: String| async move { req });

        tokio::spawn(async move {
            responder
                .process_requests_with_strategy(ConcurrencyStrategy::Fixed(16))
                .await
                .unwrap();
        });

        let response = client
            .request(black_box("Hello, world!".to_string()))
            .await
            .unwrap();
        black_box(response);
    });
}

/// Benchmark for processing requests with dynamic concurrency.
fn bench_dynamic_concurrency() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async move {
        let (client, responder_builder) = channel(64, Duration::from_secs(2));
        let responder = responder_builder.build(|req: String| async move { req });

        tokio::spawn(async move {
            responder
                .process_requests_with_strategy(ConcurrencyStrategy::Dynamic(4, 16))
                .await
                .unwrap();
        });

        let response = client
            .request(black_box("Hello, world!".to_string()))
            .await
            .unwrap();
        black_box(response);
    });
}

/// Benchmark for sending a single request with a custom timeout.
fn bench_single_request_timeout() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async move {
        let (client, responder_builder) = channel(64, Duration::from_secs(2));
        let responder = responder_builder.build(|req: String| async move { req });

        tokio::spawn(async move {
            responder.process_requests().await.unwrap();
        });

        let response = client
            .request_timeout(
                black_box("Hello, world!".to_string()),
                Duration::from_millis(100),
            )
            .await
            .unwrap();
        black_box(response);
    });
}

/// Benchmark for sending a batch of requests with a custom timeout.
fn bench_batch_requests_timeout() {
    let rt = Runtime::new().unwrap();

    rt.block_on(async move {
        let (client, responder_builder) = channel(64, Duration::from_secs(2));
        let responder = responder_builder.build(|req: String| async move { req });

        tokio::spawn(async move {
            responder.process_requests().await.unwrap();
        });

        let requests = vec![
            black_box("Request 1".to_string()),
            black_box("Request 2".to_string()),
        ];
        let responses = client
            .request_batch_timeout(requests, Duration::from_millis(100), 4)
            .await
            .unwrap();
        black_box(responses);
    });
}

fn benchmark(c: &mut Criterion) {
    c.bench_function("single_request", |b| b.iter(bench_single_request))
        .bench_function("batch_requests", |b| b.iter(bench_batch_requests))
        .bench_function("fixed_concurrency", |b| {
            b.iter(bench_fixed_concurrency)
        })
        .bench_function("dynamic_concurrency", |b| {
            b.iter( bench_dynamic_concurrency)
        })
        .bench_function("single_request_timeout", |b| {
            b.iter( bench_single_request_timeout)
        })
        .bench_function("batch_requests_timeout", |b| {
            b.iter(bench_batch_requests_timeout)
        });
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
