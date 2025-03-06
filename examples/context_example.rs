use async_duplex_channel::*;
use std::time::Duration;
use tokio::runtime::Runtime;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::*};

fn main() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let (client, responder_builder) = channel(64, Duration::from_secs(2));

        let num = Arc::new(AtomicU64::new(0u64));
        let hdl = move |req: String| {
            let num = num.clone();
            async move {
                format!("{}, {}", num.fetch_add(1, Relaxed), req)
            }
        };
        let responder = responder_builder.build(hdl);

        tokio::spawn(async move {
            responder.process_requests().await.unwrap();
        });

        let resp_1 = client.request("Hello, world!".to_string()).await.unwrap();
        println!("Response: {}", resp_1);

        let resp_2 = client.request("Good Morning!".into()).await.unwrap();
        println!("Response: {}", resp_2);

        let resp_seq = client.request_batch(["A", "B", "C", "D", "E"].map(|s| s.to_string()), 4);
        println!("Response Seq: {:?}", resp_seq.await.unwrap());
    });
}
