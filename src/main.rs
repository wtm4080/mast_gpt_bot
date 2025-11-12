mod mastodon;
mod openai_api;
mod util;
mod notification_stream;
mod config;
mod conversation_store;

use crate::conversation_store::ConversationStore;
use anyhow::Result;
use config::BotConfig;
use mastodon::post_status;
use openai_api::generate_free_toot;
use std::sync::Arc;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    let config = BotConfig::from_env()?;
    println!("config = {:?}", config.redacted());

    let conv_store = ConversationStore::new(&config.bot_db_path)?;
    let conv_store = Arc::new(conv_store);

    println!("Starting Mastodon GPT bot (streaming mode)…");
    println!("Streaming URL base: {}", config.streaming_base_url);

    let client = reqwest::Client::new();

    // 1. 通知ストリーム → メンションに返信
    let client_stream = client.clone();
    let config_stream = config.clone();
    let conv_store_stream = conv_store.clone();

    let stream_task = tokio::spawn(async move {
        if let Err(e) = notification_stream::run_notification_stream(
            &client_stream,
            &config_stream,
            conv_store_stream,
        )
            .await
        {
            eprintln!("Streaming task error: {:?}", e);
        }
    });

    // 2. 1時間ごとに自由トゥート
    let client_free = client.clone();
    let config_free = config.clone();
    let interval_free = config.free_toot_interval;

    let free_toot_task = tokio::spawn(async move {
        loop {
            sleep(interval_free).await;

            println!("[free toot] Generating…");
            if let Err(e) = do_free_toot(&client_free, &config_free).await {
                eprintln!("[free toot] Error: {:?}", e);
            }
        }
    });

    let _ = tokio::join!(stream_task, free_toot_task);
    Ok(())
}

async fn do_free_toot(client: &reqwest::Client, config: &BotConfig) -> Result<()> {
    let text = generate_free_toot(
        client,
        &config.openai_model,
        &config.openai_api_key,
        config.free_toot_temperature,
    )
        .await?;

    println!("[free toot] {}", text);

    post_status(
        client,
        &config.mastodon_base,
        &config.mastodon_token,
        &text,
        &config.visibility.to_string(),
    )
        .await?;

    Ok(())
}

