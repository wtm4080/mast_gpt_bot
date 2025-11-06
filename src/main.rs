///! エントリポイント。設定読み込み＆メインループだけ

// src/main.rs
mod mastodon;
mod openai_api;
mod util;
mod notification_stream;

use anyhow::Result;
use dotenvy::dotenv;
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

    // Streaming API のベース URL
    // 例: wss://kirishima.cloud/api/v1/streaming
    // 設定されていなければ、MASTODON_BASE_URL から雑に変換
    let streaming_base_url = env::var("MASTODON_STREAMING_URL").unwrap_or_else(|_| {
        let base = mastodon_base.trim_end_matches('/');
        if base.starts_with("https://") {
            format!("wss://{}{}", &base["https://".len()..], "/api/v1/streaming")
        } else if base.starts_with("http://") {
            format!("ws://{}{}", &base["http://".len()..], "/api/v1/streaming")
        } else {
            // 最悪そのまま wss:// を前に付ける
            format!("wss://{}{}", base, "/api/v1/streaming")
        }
    });

    let client = reqwest::Client::builder()
        .user_agent("mastodon-gpt-bot/0.2")
        .build()?;

    println!("Starting Mastodon GPT bot (streaming mode)…");
    println!("Streaming URL base: {}", streaming_base_url);

    // 1. 通知ストリーム → メンションに返信
    let client_stream = client.clone();
    let mastodon_base_stream = mastodon_base.clone();
    let mastodon_token_stream = mastodon_token.clone();
    let openai_model_stream = openai_model.clone();
    let openai_api_key_stream = openai_api_key.clone();
    let streaming_url_stream = streaming_base_url.clone();

    let stream_task = tokio::spawn(async move {
        if let Err(e) = run_notification_stream(
            &client_stream,
            &streaming_url_stream,
            &mastodon_base_stream,
            &mastodon_token_stream,
            &openai_model_stream,
            &openai_api_key_stream,
        )
            .await
        {
            eprintln!("run_notification_stream exited with error: {:?}", e);
        }
    });

    // 2. 1時間ごとに自由トゥート
    let client_free = client.clone();
    let mastodon_base_free = mastodon_base.clone();
    let mastodon_token_free = mastodon_token.clone();
    let openai_model_free = openai_model.clone();
    let openai_api_key_free = openai_api_key.clone();
    let post_visibility_free = post_visibility.clone();

    let free_toot_task = tokio::spawn(async move {
        loop {
            // 起動から1時間待つならここを sleep(3600) にする
            sleep(Duration::from_secs(3600)).await;

            println!("[free toot] Generating…");
            match generate_free_toot(&client_free, &openai_model_free, &openai_api_key_free).await
            {
                Ok(text) => {
                    println!("[free toot] {}", text);
                    if let Err(e) = post_status(
                        &client_free,
                        &mastodon_base_free,
                        &mastodon_token_free,
                        &text,
                        &post_visibility_free,
                    )
                        .await
                    {
                        eprintln!("[free toot] Failed to post: {:?}", e);
                    }
                }
                Err(e) => {
                    eprintln!("[free toot] Failed to generate: {:?}", e);
                }
            }
        }
    });

    // 両方のタスクが走り続けるようにする
    let _ = tokio::join!(stream_task, free_toot_task);

    Ok(())
}
