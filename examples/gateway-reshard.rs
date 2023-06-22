use futures_util::StreamExt;
use std::{env, time::Duration};
use tokio::time;
use twilight_gateway::{
    stream::{self, ShardEventStream, ShardMessageStream},
    Config, ConfigBuilder, Intents, Shard, ShardId,
};
use twilight_http::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize the tracing subscriber.
    tracing_subscriber::fmt::init();

    let token = env::var("DISCORD_TOKEN")?;
    let client = Client::new(token.clone());
    let config = Config::new(token, Intents::GUILDS);
    let config_callback = |_, builder: ConfigBuilder| builder.build();

    let mut shards = stream::create_recommended(&client, config.clone(), config_callback)
        .await?
        .collect::<Vec<_>>();

    loop {
        // Run the two futures concurrently, returning when the first branch
        // completes and cancels the other one.
        tokio::select! {
            _ = runner(shards) => break,
            new_shards = reshard(&client, config.clone(), config_callback) => {
                shards = new_shards?;
            }
        }
    }

    Ok(())
}

// Instrument to differentiate between the logs produced here and in `reshard`.
#[tracing::instrument(skip_all)]
async fn runner(mut shards: Vec<Shard>) {
    let mut stream = ShardEventStream::new(shards.iter_mut());

    while let Some((shard, event)) = stream.next().await {
        let event = match event {
            Ok(event) => event,
            Err(source) => {
                tracing::warn!(?source, "error receiving event");

                if source.is_fatal() {
                    break;
                }

                continue;
            }
        };

        tracing::debug!(?event, shard = ?shard.id(), "received event");
    }
}

// Instrument to differentiate between the logs produced here and in `runner`.
#[tracing::instrument(skip_all)]
async fn reshard(
    client: &Client,
    config: Config,
    config_callback: impl Fn(ShardId, ConfigBuilder) -> Config,
) -> anyhow::Result<Vec<Shard>> {
    // Reshard every eight hours. This is an arbitrary number.
    const RESHARD_DURATION: Duration = Duration::from_secs(60 * 60 * 8);

    time::sleep(RESHARD_DURATION).await;

    let mut shards = stream::create_recommended(client, config, config_callback)
        .await?
        .collect::<Vec<_>>();

    // Before swapping the old and new list of shards, try to identify them.
    // Don't try too hard, however, as large bots may never have all shards
    // identified at the same time.
    let mut identified = vec![0; shards.len()];
    let mut stream = ShardMessageStream::new(shards.iter_mut());

    while identified.iter().sum::<usize>() < (identified.len() * 3) / 4 {
        match stream.next().await {
            Some((_, Err(source))) => {
                tracing::warn!(?source, "error receiving message");

                if source.is_fatal() {
                    anyhow::bail!(source);
                }
            }
            Some((shard, _)) => {
                identified[shard.id().number() as usize] = shard.status().is_identified().into();
            }
            _ => {}
        }
    }

    drop(stream);
    Ok(shards)
}
