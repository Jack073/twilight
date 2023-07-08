mod common;

use common::{different_id_is_parallel, multi_bucket, reset_after_refills, same_id_is_serial};
use std::{
    future::{poll_fn, Future},
    pin::Pin,
};
use tokio::time::{advance, Duration, Instant};
use twilight_gateway_queue::{InMemoryQueue, Queue, IDENTIFY_DELAY};

#[tokio::test]
async fn disabled_is_instant() {
    let queue = InMemoryQueue::new(0, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(0);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(now.elapsed() < IDENTIFY_DELAY, "did not run instantly");
}

#[tokio::test]
async fn update_fills_bucket() {
    let queue = InMemoryQueue::new(1, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    let now = Instant::now();

    // Background task not run due to single-threaded runtime.
    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(1);
    queue.update(2, 1000, Duration::from_secs(60 * 60 * 24), 1000);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(now.elapsed() < IDENTIFY_DELAY, "ran serially");
}

#[tokio::test(start_paused = true)]
async fn integration() {
    let queue = InMemoryQueue::new(1, 1000, Duration::from_secs(60 * 60 * 24), 1000);

    same_id_is_serial(queue.clone()).await;

    advance(IDENTIFY_DELAY).await;
    queue.update(2, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    different_id_is_parallel(queue.clone()).await;

    advance(IDENTIFY_DELAY).await;
    queue.update(1, 0, Duration::from_secs(60), 1);
    reset_after_refills(queue.clone(), Duration::from_secs(60)).await;

    advance(IDENTIFY_DELAY).await;
    queue.update(4, 1000, Duration::from_secs(60 * 60 * 24), 1000);
    multi_bucket(queue).await;
}
