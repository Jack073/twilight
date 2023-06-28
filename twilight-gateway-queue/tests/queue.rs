use std::{
    future::{poll_fn, Future},
    pin::Pin,
};
use tokio::time::{advance, timeout, Duration, Instant};
use twilight_gateway_queue::{InMemoryQueue, Queue, IDENTIFY_INTERVAL};

async fn same_id_is_serial(queue: impl Queue) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(0);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(now.elapsed() >= IDENTIFY_INTERVAL, "ran concurrently");
}

/// Requires a queue with `max_concurrency` > 1.
async fn different_id_is_parallel(queue: impl Queue) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(1);

    tokio::select! {
        _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)) => {
            _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;
            assert!(now.elapsed() < IDENTIFY_INTERVAL, "ran serially");
        }
        _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)) => panic!("not started in order"),
    }
}

/// Requires a queue with `total` >= 1.
async fn reset_after_refills(queue: impl Queue, reset_after: Duration) {
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
async fn multi_bucket(queue: impl Queue) {
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

#[tokio::test]
async fn memory_disabled_is_instant() {
    let queue = InMemoryQueue::new(0, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(0);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(now.elapsed() < IDENTIFY_INTERVAL, "did not run instantly");
}

#[tokio::test]
async fn memory_update_fills_bucket() {
    let queue = InMemoryQueue::new(1, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    let now = Instant::now();

    // Background task not run due to single-threaded runtime.
    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(1);
    queue.update(2, 1000, Duration::from_secs(60 * 60 * 24), 1000);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(now.elapsed() < IDENTIFY_INTERVAL, "ran serially");
}

#[tokio::test(start_paused = true)]
async fn memory_integration() {
    let queue = InMemoryQueue::new(1, 1000, Duration::from_secs(60 * 60 * 24), 1000);

    same_id_is_serial(queue.clone()).await;

    advance(IDENTIFY_INTERVAL).await;
    queue.update(2, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    different_id_is_parallel(queue.clone()).await;

    advance(IDENTIFY_INTERVAL).await;
    queue.update(1, 0, Duration::from_secs(60), 1);
    reset_after_refills(queue.clone(), Duration::from_secs(60)).await;

    advance(IDENTIFY_INTERVAL).await;
    queue.update(4, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    multi_bucket(queue).await;
}
