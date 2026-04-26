use crate::config::BotConfig;
use crate::conversation_store::ConversationStore;
use anyhow::{Context as AnyhowContext, Result};
use futures_util::StreamExt;
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

use super::handler::handle_ws_text;
use super::recoverable::{RecoverableFailure, log_recoverable_error};

pub async fn run_notification_stream(
    client: &reqwest::Client,
    config: &BotConfig,
    conv_store: Arc<ConversationStore>,
) -> Result<()> {
    loop {
        println!("Connecting to Mastodon streaming API…");

        let streaming_base_url = &config.streaming_base_url;
        let mastodon_token = &config.mastodon_access_token;

        match connect_stream(streaming_base_url, mastodon_token).await {
            Ok((mut ws_read, url)) => {
                println!("Connected to streaming API: {}", url);

                while let Some(msg) = ws_read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Err(e) = handle_ws_text(client, config, &conv_store, &text).await
                            {
                                log_recoverable_error(RecoverableFailure::HandleStreamMessage, &e);
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
                            log_recoverable_error(RecoverableFailure::WebSocket, &e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                log_recoverable_error(RecoverableFailure::ConnectStreamingApi, &e);
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
    let url = streaming_user_url(streaming_base_url, token)?;

    let (ws_stream, _resp) =
        connect_async(url.as_str()).await.context("Failed to connect WebSocket")?;

    let (_write, read) = ws_stream.split();
    Ok((read, url))
}

fn streaming_user_url(streaming_base_url: &str, token: &str) -> Result<Url> {
    let mut url = Url::parse(streaming_base_url).context("Failed to parse streaming base URL")?;

    // 認証付きで user ストリームに接続
    url.set_query(Some(&format!("stream=user&access_token={}", token)));

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaming_user_url_adds_user_stream_and_access_token_query() {
        let url = streaming_user_url("wss://mastodon.example/api/v1/streaming", "mastodon-token")
            .unwrap();

        assert_eq!(
            url.as_str(),
            "wss://mastodon.example/api/v1/streaming?stream=user&access_token=mastodon-token"
        );
    }

    #[tokio::test]
    async fn connect_stream_surfaces_connection_failure() {
        let url = crate::test_support::closed_local_ws_url("/api/v1/streaming");

        let err = match connect_stream(&url, "mastodon-token").await {
            Ok(_) => panic!("expected connect_stream to fail"),
            Err(err) => err,
        };
        let message = format!("{err:#}");

        assert!(message.contains("Failed to connect WebSocket"));
    }

    #[tokio::test]
    async fn connect_stream_surfaces_http_handshake_failure() {
        let server = crate::test_support::MockHttpServer::respond("500 Internal Server Error", "");
        let url = server.base_url().replacen("http://", "ws://", 1);

        let err = match connect_stream(&url, "mastodon-token").await {
            Ok(_) => panic!("expected connect_stream to fail"),
            Err(err) => err,
        };
        let message = format!("{err:#}");

        assert!(message.contains("Failed to connect WebSocket"));
    }
}
