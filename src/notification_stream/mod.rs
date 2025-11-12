use crate::config::BotConfig;
use crate::mastodon::{fetch_status_context, post_reply, Notification};
use crate::openai_api::generate_reply;
use crate::util::strip_html;

use anyhow::{Context as AnyhowContext, Result};
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

mod context;
mod rate_limit;

use context::format_conversation_context;
use rate_limit::wait_for_rate_limit;

pub async fn run_notification_stream(
    client: &reqwest::Client,
    config: &BotConfig,
) -> Result<()> {
    loop {
        println!("Connecting to Mastodon streaming API…");

        let streaming_base_url = &config.streaming_base_url;
        let mastodon_token = &config.mastodon_token;

        match connect_stream(streaming_base_url, mastodon_token).await {
            Ok((mut ws_read, url)) => {
                println!("Connected to streaming API: {}", url);

                while let Some(msg) = ws_read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = handle_ws_text(client, config, &text).await {
                                eprintln!("Error handling stream message: {:?}", e);
                            }
                        }
                        Ok(Message::Ping(_)) => {
                            // tungstenite が自動で Pong 返してくれるので放置
                        }
                        Ok(Message::Close(frame)) => {
                            println!("WebSocket closed: {:?}", frame);
                            break;
                        }
                        Ok(_other) => {
                            // Binary などは無視
                        }
                        Err(e) => {
                            eprintln!("WebSocket error: {:?}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to connect streaming API: {:?}", e);
            }
        }

        println!("Streaming connection lost. Reconnecting in 5 seconds…");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn connect_stream(
    streaming_base_url: &str,
    token: &str,
) -> Result<(
    impl futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>,
    Url,
)> {
    let mut url = Url::parse(streaming_base_url)
        .context("Failed to parse streaming base URL")?;

    // 認証付きで user ストリームに接続
    url.set_query(Some(&format!("stream=user&access_token={}", token)));

    let (ws_stream, _resp) = connect_async(url.as_str())
        .await
        .context("Failed to connect WebSocket")?;

    let (_write, read) = ws_stream.split();
    Ok((read, url))
}

async fn handle_ws_text(
    client: &reqwest::Client,
    config: &BotConfig,
    text: &str,
) -> Result<()> {
    let ev: StreamEvent = serde_json::from_str(text)
        .context("Failed to parse stream event JSON")?;

    if ev.event != "notification" {
        return Ok(());
    }

    let payload = match ev.payload {
        Some(ref p) => p,
        None => return Ok(()),
    };

    let notif: Notification = serde_json::from_str(payload)
        .context("Failed to parse notification payload")?;

    if notif.notif_type != "mention" {
        return Ok(());
    }

    // bot 同士のリプ合戦防止
    if notif.account.bot.unwrap_or(false) {
        println!(
            "Skip mention from bot account @{} (id={})",
            notif.account.acct, notif.id
        );
        return Ok(());
    }

    let status = match notif.status {
        Some(ref s) => s,
        None => return Ok(()),
    };

    let plain = strip_html(&status.content);
    println!("(stream) Mention from @{}: {}", notif.account.acct, plain);

    // 会話コンテキスト取得
    let conversation_context = match fetch_status_context(
        client,
        &config.mastodon_base,
        &config.mastodon_token,
        &status.id,
    )
        .await
    {
        Ok(ctx) => {
            let ctx_text = format_conversation_context(&ctx, status);
            if ctx_text.is_empty() {
                None
            } else {
                Some(ctx_text)
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch status context: {:?}", e);
            None
        }
    };

    // レートリミット（連続で投げすぎないように）
    wait_for_rate_limit(config.reply_min_interval_ms).await;

    match generate_reply(
        client,
        &config.openai_model,
        &config.openai_api_key,
        &plain,
        conversation_context.as_deref(),
    )
        .await
    {
        Ok(reply_text) => {
            println!(" -> Reply: {}", reply_text);
            if let Err(e) = post_reply(
                client,
                &config.mastodon_base,
                &config.mastodon_token,
                status,
                &notif.account.acct,
                &reply_text,
            )
                .await
            {
                eprintln!("Failed to post reply: {:?}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to generate reply: {:?}", e);
        }
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct StreamEvent {
    event: String,
    payload: Option<String>,
}
