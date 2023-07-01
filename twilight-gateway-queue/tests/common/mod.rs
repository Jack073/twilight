use std::{
    future::{poll_fn, Future},
    pin::Pin,
};
use tokio::time::{timeout, Duration, Instant};
use twilight_gateway_queue::{Queue, IDENTIFY_INTERVAL};

pub async fn same_id_is_serial(queue: impl Queue) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(0);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(now.elapsed() >= IDENTIFY_INTERVAL, "ran concurrently");
}

/// Requires a queue with `max_concurrency` > 1.
pub async fn different_id_is_parallel(queue: impl Queue) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(1);

    tokio::select! {
        biased;
        _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)) => panic!("not started in order"),
        _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)) => {
            _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;
            assert!(now.elapsed() < IDENTIFY_INTERVAL, "ran serially");
        }
    }
}

/// Requires a queue with `total` >= 1.
pub async fn reset_after_refills(queue: impl Queue, reset_after: Duration) {
    let mut t1 = queue.enqueue(0);

    let duration = reset_after + Duration::from_nanos(1);
    assert!(
        timeout(duration, poll_fn(|cx| Pin::new(&mut t1).poll(cx)))
            .await
            .is_ok(),
        "did not refill in time"
    );
}

/// Requires a queue with `max_concurrency` >= 4.
pub async fn multi_bucket(queue: impl Queue) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(1);
    let mut t3 = queue.enqueue(3);
    let mut t4 = queue.enqueue(3);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t3).poll(cx)).await;

    assert!(now.elapsed() < IDENTIFY_INTERVAL, "ran concurrently");

    _ = poll_fn(|cx| Pin::new(&mut t4).poll(cx)).await;

    assert!(now.elapsed() >= IDENTIFY_INTERVAL, "ran serially");
}
