#![doc = include_str!("../README.md")]
#![warn(
    clippy::missing_const_for_fn,
    clippy::missing_docs_in_private_items,
    clippy::pedantic,
    missing_docs,
    unsafe_code
)]
#![allow(clippy::must_use_candidate)]

mod in_memory;

pub use in_memory::InMemoryQueue;

use std::fmt::Debug;
use tokio::{sync::oneshot, time::Duration};

/// Period between buckets.
pub const IDENTIFY_INTERVAL: Duration = Duration::from_secs(5);

/// Abstraction for types processing gateway identify requests.
///
/// For convenience in twilight-gateway, implementors must also implement
/// [`Debug`], [`Send`], and [`Sync`].
pub trait Queue: Debug + Send + Sync {
    /// Enqueue a shard with this ID.
    ///
    /// Send `()` to signal the shard to proceed. Note that shards may have
    /// dropped the receiver prior.
    ///
    /// Closing the channel should causes the shard to requeue.
    fn enqueue(&self, id: u32) -> oneshot::Receiver<()>;
}

impl<T> Queue for &T
where
    T: Queue,
{
    fn enqueue(&self, shard: u32) -> oneshot::Receiver<()> {
        (**self).enqueue(shard)
    }
}

#[cfg(test)]
mod tests {
    use super::Queue;
    use static_assertions::{assert_impl_all, assert_obj_safe};
    use std::fmt::Debug;

    assert_impl_all!(dyn Queue: Debug, Send, Sync);
    assert_obj_safe!(Queue);
}
