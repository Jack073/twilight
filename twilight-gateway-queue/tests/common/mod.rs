use std::{
    future::{poll_fn, Future},
    pin::Pin,
};
use tokio::time::{advance, Duration, Instant};
use twilight_gateway_queue::{Queue, IDENTIFY_DELAY, LIMIT_PERIOD};

pub async fn same_id_is_serial(queue: impl Queue) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(0);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(now.elapsed() >= IDENTIFY_DELAY, "ran concurrently");
}

/// Requires a queue with `max_concurrency` > 1.
pub async fn different_id_is_parallel(queue: impl Queue) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(1);
    let mut t2 = queue.enqueue(0);

    tokio::select! {
        biased;
        _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)) => panic!("not started in order"),
        _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)) => {
            _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;
            assert!(now.elapsed() < IDENTIFY_DELAY, "ran serially");
        }
    }
}

/// Requires a queue with `total` >= 1.
pub async fn reset_after_refills(queue: impl Queue, reset_after: Duration) {
    let now = Instant::now();

    let mut t1 = queue.enqueue(0);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;

    assert!(
        (now.elapsed().as_secs_f64() - reset_after.as_secs_f64()).abs() <= 1e-2,
        "did not refill in time"
    );
}

/// Requires a fresh queue with `remaining` of 1.
pub async fn reset_after_started(queue: impl Queue) {
    advance(LIMIT_PERIOD / 2).await;

    let mut t1 = queue.enqueue(0);
    let mut t2 = queue.enqueue(0);

    _ = poll_fn(|cx| Pin::new(&mut t1).poll(cx)).await;

    let now = Instant::now();

    _ = poll_fn(|cx| Pin::new(&mut t2).poll(cx)).await;

    assert!(
        (dbg!(now.elapsed().as_secs_f64()) - dbg!(LIMIT_PERIOD.as_secs_f64())).abs() <= 1e-2,
        "queue misstimed remaining refill"
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

    assert!(now.elapsed() < IDENTIFY_DELAY, "ran serially");

    _ = poll_fn(|cx| Pin::new(&mut t4).poll(cx)).await;

    assert!(now.elapsed() >= IDENTIFY_DELAY, "ran concurrently");
}
