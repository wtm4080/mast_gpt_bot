///! エントリポイント。設定読み込み＆メインループだけ

// src/main.rs
mod mastodon;
mod openai_api;
mod util;
mod notification_stream;
mod config;

use anyhow::Result;
use dotenvy::dotenv;
use config::BotConfig;
use mastodon::post_status;
use notification_stream::run_notification_stream;
use openai_api::generate_free_toot;
use std::env;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let mastodon_base =
        env::var("MASTODON_BASE_URL").expect("MASTODON_BASE_URL is not set");
    let mastodon_token =
        env::var("MASTODON_ACCESS_TOKEN").expect("MASTODON_ACCESS_TOKEN is not set");
    let openai_api_key =
        env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set");
    let openai_model =
        env::var("OPENAI_MODEL").expect("OPENAI_MODEL is not set (fine-tuned model name)");

    // 自由トゥートの公開範囲
    let post_visibility =
        env::var("MASTODON_POST_VISIBILITY").unwrap_or_else(|_| "public".to_string());

    let free_toot_interval_secs: u64 = env::var("FREE_TOOT_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(3600); // デフォルトは 1時間

    let reply_min_interval_ms: u64 = env::var("REPLY_MIN_INTERVAL_MILLIS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1000); // デフォルト 1000ms = 1秒

    let reply_temperature: f32 = env::var("REPLY_TEMPERATURE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.6);

    let free_toot_temperature: f32 = env::var("FREE_TOOT_TEMPERATURE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.7);

    // Streaming API のベース URL
    let streaming_base_url = env::var("MASTODON_STREAMING_URL").unwrap_or_else(|_| {
        let base = mastodon_base.trim_end_matches('/');
        if base.starts_with("https://") {
            format!("wss://{}{}", &base["https://".len()..], "/api/v1/streaming")
        } else if base.starts_with("http://") {
            format!("ws://{}{}", &base["http://".len()..], "/api/v1/streaming")
        } else {
            format!("wss://{}{}", base, "/api/v1/streaming")
        }
    });

    // ★ 共通設定を構造体にまとめる
    let config = BotConfig {
        mastodon_base,
        mastodon_token,
        openai_model,
        openai_api_key,
        post_visibility,
        streaming_base_url,
        reply_min_interval_ms,
        reply_temperature,
        free_toot_temperature,
    };

    let client = reqwest::Client::builder()
        .user_agent("mastodon-gpt-bot/0.2")
        .build()?;

    println!("Starting Mastodon GPT bot (streaming mode)…");
    println!("Streaming URL base: {}", config.streaming_base_url);

    // 1. 通知ストリーム → メンションに返信
    let client_stream = client.clone();
    let config_stream = config.clone();

    let stream_task = tokio::spawn(async move {
        if let Err(e) = run_notification_stream(&client_stream, &config_stream).await {
            eprintln!("run_notification_stream exited with error: {:?}", e);
        }
    });

    // 2. 1時間ごとに自由トゥート
    let client_free = client.clone();
    let config_free = config.clone();
    let interval_free = free_toot_interval_secs;

    let free_toot_task = tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(interval_free)).await;

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
        &config.post_visibility,
    )
        .await?;

    Ok(())
}

