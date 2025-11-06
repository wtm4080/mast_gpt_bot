///! notificationストリームを購読して、適宜返信する

// src/notification_stream.rs
use crate::mastodon::{post_reply, Notification};
use crate::openai_api::generate_reply;
use crate::util::strip_html;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

/// WebSocket から来る 1 メッセージ分
#[derive(Debug, Deserialize)]
struct StreamEvent {
    event: String,
    payload: Option<String>,
    // Mastodon 3.3+ だと stream: ["user","notification"] みたいなのも付いてくる
    _stream: Option<Vec<String>>,
}

/// streaming API に接続して、notification イベントを処理し続ける
pub async fn run_notification_stream(
    client: &reqwest::Client,
    streaming_base_url: &str,
    mastodon_base_url: &str,
    mastodon_token: &str,
    openai_model: &str,
    openai_api_key: &str,
) -> Result<()> {
    loop {
        println!("Connecting to Mastodon streaming API…");

        match connect_stream(streaming_base_url, mastodon_token).await {
            Ok((mut ws_read, _url)) => {
                println!("Connected to streaming API.");

                while let Some(msg) = ws_read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = handle_ws_text(
                                client,
                                &text,
                                mastodon_base_url,
                                mastodon_token,
                                openai_model,
                                openai_api_key,
                            )
                                .await
                            {
                                eprintln!("Error handling stream message: {:?}", e);
                            }
                        }
                        Ok(Message::Ping(_)) => {
                            // tokio-tungstenite が勝手に Pong 返してくれるので放置でもOK
                        }
                        Ok(Message::Close(frame)) => {
                            println!("WebSocket closed: {:?}", frame);
                            break;
                        }
                        Ok(_other) => {
                            // Binary などは無視
                            // println!("WS other msg: {:?}", _other);
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

        // 切断されたら数秒待って再接続
        println!("Streaming connection lost. Reconnecting in 5 seconds…");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// WebSocket 接続を張る
async fn connect_stream(
    streaming_base_url: &str,
    mastodon_token: &str,
) -> Result<(impl futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>, Url)>
{
    // streaming_base_url 例:
    //   wss://kirishima.cloud/api/v1/streaming
    //
    // ここに access_token と stream=user:notification を付ける
    let mut url = Url::parse(streaming_base_url)
        .context("Invalid MASTODON_STREAMING_URL")?;

    {
        let mut qp = url.query_pairs_mut();
        qp.append_pair("access_token", mastodon_token);
        qp.append_pair("stream", "user:notification");
    }

    let (ws_stream, _resp) = connect_async(url.as_str())
        .await
        .context("connect_async failed")?;

    // 今回は送るものはないので read だけ返す
    let (_write, read) = ws_stream.split();
    Ok((read, url))
}

/// WebSocket の Text メッセージ1本を処理
async fn handle_ws_text(
    client: &reqwest::Client,
    text: &str,
    mastodon_base_url: &str,
    mastodon_token: &str,
    openai_model: &str,
    openai_api_key: &str,
) -> Result<()> {
    let ev: StreamEvent = serde_json::from_str(text)
        .context("Failed to parse stream event JSON")?;

    // notification 以外 (update, delete, filters_changed など) は無視
    if ev.event != "notification" {
        return Ok(());
    }

    let payload = match ev.payload {
        Some(ref p) => p,
        None => return Ok(()),
    };

    // payload は Notification JSON が文字列として入ってるので、もう一回 parse
    let notif: Notification = serde_json::from_str(payload)
        .context("Failed to parse notification payload")?;

    // mention 以外はスキップ
    if notif.notif_type != "mention" {
        return Ok(());
    }

    // bot アカウントからのメンションはスキップ
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

    // OpenAI で返信を生成
    match generate_reply(client, openai_model, openai_api_key, &plain).await {
        Ok(reply_text) => {
            println!(" -> Reply: {}", reply_text);
            if let Err(e) = post_reply(
                client,
                mastodon_base_url,
                mastodon_token,
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

