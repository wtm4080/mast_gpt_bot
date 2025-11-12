///! notificationストリームを購読して、適宜返信する

use crate::config::BotConfig;
use crate::mastodon::{
    post_reply,
    fetch_status_context,
    Notification,
    Status,
    StatusContext,
};
use crate::openai_api::generate_reply;
use crate::util::strip_html;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use once_cell::sync::Lazy;

// グローバルな「最後にOpenAIへ投げた時刻」
static LAST_REPLY_AT: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));

async fn wait_for_rate_limit(min_interval_ms: u64) {
    use tokio::time::sleep;

    let mut guard = LAST_REPLY_AT.lock().await;
    let min_interval = Duration::from_millis(min_interval_ms);

    if let Some(last) = *guard {
        let elapsed = last.elapsed();
        if elapsed < min_interval {
            let wait = min_interval - elapsed;
            // 必要なぶんだけsleep
            sleep(wait).await;
        }
    }

    // 「今回投げた時刻」を更新
    *guard = Some(Instant::now());
}

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
    config: &BotConfig,
) -> Result<()> {
    loop {
        println!("Connecting to Mastodon streaming API…");

        // config から必要な値を取り出す
        let streaming_base_url = &config.streaming_base_url;
        let mastodon_token = &config.mastodon_token;

        match connect_stream(streaming_base_url, mastodon_token).await {
            Ok((mut ws_read, _url)) => {
                println!("Connected to streaming API.");

                while let Some(msg) = ws_read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = handle_ws_text(
                                client,
                                &text,
                                config, // ★ ここに config を渡す
                            )
                                .await
                            {
                                eprintln!("Error handling stream message: {:?}", e);
                            }
                        }
                        Ok(Message::Ping(_)) => {
                            // Pong 自動返信に任せる
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
    config: &BotConfig,
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

    // ★ 会話コンテキストを取得（失敗したら諦めて単発扱い）
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

    // ここでレートリミットに従う（必要ならsleep）
    wait_for_rate_limit(config.reply_min_interval_ms).await;

    match generate_reply(
        client,
        &config.openai_model,
        &config.openai_api_key,
        &plain,
        conversation_context.as_deref(),  // ★ ここで渡す
    )
        .await {
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

fn format_conversation_context(ctx: &StatusContext, current: &Status) -> String {
    // ancestors は古い順で返ってくる想定なので、直近数件だけに絞る
    let ancestors = &ctx.ancestors;
    let max_back = 10; // 遡って見る最大件数
    let start = if ancestors.len() > max_back {
        ancestors.len() - max_back
    } else {
        0
    };

    let mut lines = Vec::new();

    for s in &ancestors[start..] {
        let text = strip_html(&s.content);
        if !text.is_empty() {
            lines.push(text);
        }
    }

    // 最後に「今の投稿」を入れる
    let current_text = strip_html(&current.content);
    if !current_text.is_empty() {
        lines.push(current_text);
    }

    // 「古い順」のリストとして整形
    // 例:
    // - 前の人: ...
    // - 自分: ...
    // - 相手: ...
    lines
        .into_iter()
        .map(|t| format!("- {}", t))
        .collect::<Vec<_>>()
        .join("\n")
}
