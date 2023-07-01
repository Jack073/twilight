# twilight-gateway-queue

Rate limiting functionality for gateway `IDENTIFY` commands.

Discord allows bot's shards to send a limited amount of `IDENTIFY` commands
every 5 seconds, with a daily limit from 1000 to 2000 commands, and invalidates
*all* shard sessions upon exceeding it. Each identify interval may be filled by
shards' IDs modulo `max_concurrency` and such a set of shards is called a
bucket. See [Discord Docs/Sharding].

To coordinate this, a [`Queue`] should process each identify request and shards
should wait for its signal to proceed before continuing and otherwise retry. The
provided [`InMemoryQueue`] never fails or cancels requests and is therefore a
good starting point for custom implementations. For most cases, simply wrapping
[`InMemoryQueue`] is be enough to add new capabilities such as multi-process
support, see [`gateway-queue`] and [`gateway-queue-http`] for a HTTP server and
client implementation, respectively. Integration tests can be found
[here](https://github.com/twilight-rs/twilight/blob/main/twilight-gateway-queue/tests/common/mod.rs).

[Discord Docs/Sharding]: https://discord.com/developers/docs/topics/gateway#sharding
[`gateway-queue`]: https://github.com/twilight-rs/gateway-queue
[`gateway-queue-http`]: https://github.com/twilight-rs/twilight/blob/main/examples/gateway-queue-http.rs
