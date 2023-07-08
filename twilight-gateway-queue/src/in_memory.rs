//! Memory based [`Queue`] implementation and supporting items.

use super::{Queue, IDENTIFY_DELAY, LIMIT_PERIOD};
use std::{collections::VecDeque, fmt::Debug, iter};
use tokio::{
    sync::{mpsc, oneshot},
    task::yield_now,
    time::{sleep_until, Duration, Instant},
};

/// Possible messages from the [`InMemoryQueue`] to the [`runner`].
#[derive(Debug)]
enum Message {
    /// Request a permit.
    Request {
        /// For this shard.
        shard: u32,
        /// Indicate readiness through this sender.
        tx: oneshot::Sender<()>,
    },
    /// Update the runner's settings.
    Update(Settings),
}

/// [`runner`]'s settings.
#[derive(Debug)]
struct Settings {
    /// The maximum number of concurrent permits to grant. `0` instantly grants
    /// all permits.
    max_concurrency: u8,
    /// Remaining daily permits.
    remaining: u16,
    /// Time until the daily permits reset.
    reset_at: Instant,
    /// The number of permits to reset to.
    total: u16,
}

/// [`InMemoryQueue`]'s background task runner.
///
/// Buckets requests such that only one timer is necessary.
async fn runner(
    mut rx: mpsc::UnboundedReceiver<Message>,
    Settings {
        mut max_concurrency,
        mut remaining,
        reset_at,
        mut total,
    }: Settings,
) {
    let reset_at = sleep_until(reset_at);
    let interval = sleep_until(Instant::now());
    tokio::pin! {
        reset_at,
        interval
    };
    let create_queues = |max_concurrency: u8| {
        iter::repeat_with(VecDeque::new)
            .take(max_concurrency.into())
            .collect::<Vec<_>>()
    };
    let mut queues = create_queues(max_concurrency);

    loop {
        tokio::select! {
            biased;
            _ = &mut reset_at => {
                remaining = total;
                let previous = reset_at.deadline();
                reset_at.as_mut().reset(previous + LIMIT_PERIOD);
            }
            message = rx.recv() => {
                match message {
                    Some(Message::Request { shard, tx }) => {
                        if max_concurrency == 0 {
                            _ = tx.send(());
                        } else {
                            queues[(shard % u32::from(max_concurrency)) as usize].push_back((shard, tx));
                        }
                    }
                    Some(Message::Update(update)) => {
                        let deadline;
                        Settings {
                            max_concurrency,
                            remaining,
                            reset_at: deadline,
                            total,
                        } = update;

                        if queues.len() != max_concurrency as usize {
                            let unbalanced = queues.into_iter().flatten();
                            queues = create_queues(max_concurrency);
                            for (shard, tx) in unbalanced {
                                queues[(shard % u32::from(max_concurrency)) as usize]
                                    .push_back((shard, tx));
                            }
                        }
                        reset_at.as_mut().reset(deadline);
                    }
                    None => break,
                }
            }
            _ = &mut interval, if queues.iter().any(|queue| !queue.is_empty()) => {
                let now = Instant::now();
                let span = tracing::info_span!("bucket", capacity = %queues.len(), ?now);
                interval.as_mut().reset(now + IDENTIFY_DELAY);
                for (rate_limit_key, queue) in queues.iter_mut().enumerate() {
                    if remaining == 0 {
                        (&mut reset_at).await;
                        remaining = total;
                        let previous = reset_at.deadline();
                        reset_at.as_mut().reset(previous + LIMIT_PERIOD);

                        break;
                    }
                    while let Some((id, tx)) = queue.pop_front() {
                        let calculated_rate_limit_key = (id % u32::from(max_concurrency)) as usize;
                        debug_assert_eq!(rate_limit_key, calculated_rate_limit_key);

                        if tx.is_closed() {
                            continue;
                        }
                        _ = tx.send(());
                        tracing::trace!(parent: &span, rate_limit_key, "allowing shard {id}");
                        // Give the shard a chance to identify before continuing.
                        // Shards *must* identify in order.
                        yield_now().await;
                        remaining -= 1;
                        break;
                    }
                }
            }
        }
    }
}

/// Memory based [`Queue`] implementation backed by an efficient background task.
///
/// [`InMemoryQueue::update`] allows for dynamically changing the queue's
/// settings.
///
/// Cloning the queue is cheap and just increments a reference counter.
///
/// # Settings
///
/// `remaining` is reset to `total` after `reset_after` and then every
/// [`LIMIT_PERIOD`].
///
/// A `max_concurrency` of `0` processes all requests instantly, effectively
/// disabling the queue.
#[derive(Clone, Debug)]
pub struct InMemoryQueue {
    /// Sender to communicate with the background [task runner].
    ///
    /// [task runner]: runner
    tx: mpsc::UnboundedSender<Message>,
}

impl InMemoryQueue {
    /// Creates a new `InMemoryQueue` with custom settings.
    pub fn new(max_concurrency: u8, remaining: u16, reset_after: Duration, total: u16) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(runner(
            rx,
            Settings {
                max_concurrency,
                remaining,
                reset_at: Instant::now() + reset_after,
                total,
            },
        ));

        Self { tx }
    }

    /// Update the queue with new info from the [Get Gateway Bot] endpoint.
    ///
    /// May be regularly called as the bot joins/leaves guilds.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use twilight_gateway_queue::InMemoryQueue;
    /// # struct GetGatewayBot {
    /// #     session_start_limit: SessionStartLimit,
    /// # }
    /// # struct SessionStartLimit {
    /// #     max_concurrency: u8,
    /// #     remaining: u16,
    /// #     reset_after: u64,
    /// #     total: u16,
    /// # }
    /// # let rt = tokio::runtime::Builder::new_current_thread()
    /// #     .enable_time()
    /// #     .build()
    /// #     .unwrap();
    /// use std::time::Duration;
    ///
    /// async fn get_gateway_bot() -> GetGatewayBot {
    ///     unimplemented!()
    /// }
    ///
    /// # rt.block_on(async {
    /// # let queue = InMemoryQueue::default();
    /// let session = get_gateway_bot().await.session_start_limit;
    /// queue.update(
    ///     session.max_concurrency,
    ///     session.remaining,
    ///     Duration::from_millis(session.reset_after),
    ///     session.total,
    /// );
    /// # })
    /// ```
    ///
    /// [Get Gateway Bot]: https://discord.com/developers/docs/topics/gateway#get-gateway-bot
    pub fn update(&self, max_concurrency: u8, remaining: u16, reset_after: Duration, total: u16) {
        self.tx
            .send(Message::Update(Settings {
                max_concurrency,
                remaining,
                reset_at: Instant::now() + reset_after,
                total,
            }))
            .expect("receiver dropped after sender");
    }
}

impl Default for InMemoryQueue {
    /// Creates a new `InMemoryQueue` with Discord's default settings.
    ///
    /// Currently these are:
    ///
    /// * `max_concurrency`: 1
    /// * `remaining`: 1000
    /// * `reset_after`: [`LIMIT_PERIOD`]
    /// * `total`: 1000.
    fn default() -> Self {
        Self::new(1, 1000, LIMIT_PERIOD, 1000)
    }
}

impl Queue for InMemoryQueue {
    fn enqueue(&self, shard: u32) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Request { shard, tx })
            .expect("receiver dropped after sender");

        rx
    }
}

#[cfg(test)]
mod tests {
    use super::InMemoryQueue;
    use crate::Queue;
    use static_assertions::assert_impl_all;
    use std::fmt::Debug;

    assert_impl_all!(InMemoryQueue: Clone, Debug, Default, Send, Sync, Queue);
}
