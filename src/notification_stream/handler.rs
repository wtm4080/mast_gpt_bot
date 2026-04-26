use crate::config::BotConfig;
use crate::conversation_store::ConversationStore;
use crate::mastodon::{Notification, fetch_status_context, post_reply};
use crate::util::strip_html;
use anyhow::{Context as AnyhowContext, Result};
use std::sync::Arc;

use super::context;
use super::rate_limit::wait_for_rate_limit;

pub(crate) async fn handle_ws_text(
    client: &reqwest::Client,
    config: &BotConfig,
    conv_store: &Arc<ConversationStore>,
    text: &str,
) -> Result<()> {
    let Some(notif) = parse_mention_notification(text)? else {
        return Ok(());
    };

    let status = match notif.status.as_ref() {
        Some(s) => s,
        None => return Ok(()),
    };

    let plain = strip_html(&status.content);
    println!("(stream) Mention from @{}: {}", notif.account.acct, plain);

    let (conversation_context, thread_key) =
        fetch_conversation_context(client, config, status).await;

    let prev_response_id = load_previous_response_id(conv_store, &thread_key).await?;
    let context_for_openai =
        select_context_for_openai(conversation_context.as_deref(), prev_response_id.as_ref());

    wait_for_rate_limit(config.reply_min_interval.as_millis() as u64).await;

    match crate::openai_api::generate_reply(
        client,
        config,
        &plain,
        context_for_openai,
        prev_response_id,
    )
    .await
    {
        Ok(reply_result) => {
            println!(" -> Reply: {}", reply_result.text);
            post_generated_reply(client, config, status, &notif.account.acct, &reply_result.text)
                .await;
            save_response_id(conv_store, &thread_key, &reply_result.response_id).await;
        }
        Err(e) => {
            eprintln!("Failed to generate reply: {:?}", e);
        }
    }

    Ok(())
}

fn parse_mention_notification(text: &str) -> Result<Option<Notification>> {
    let ev: StreamEvent =
        serde_json::from_str(text).context("Failed to parse stream event JSON")?;

    if ev.event != "notification" {
        return Ok(None);
    }

    let payload = match ev.payload {
        Some(ref p) => p,
        None => return Ok(None),
    };

    let notif: Notification =
        serde_json::from_str(payload).context("Failed to parse notification payload")?;

    if notif.notif_type != "mention" {
        return Ok(None);
    }

    // bot 同士のリプ合戦防止
    if notif.account.bot.unwrap_or(false) {
        println!("Skip mention from bot account @{} (id={})", notif.account.acct, notif.id);
        return Ok(None);
    }

    Ok(Some(notif))
}

async fn fetch_conversation_context(
    client: &reqwest::Client,
    config: &BotConfig,
    status: &crate::mastodon::Status,
) -> (Option<String>, String) {
    match fetch_status_context(
        client,
        &config.mastodon_base,
        &config.mastodon_access_token,
        &status.id,
    )
    .await
    {
        Ok(ctx) => {
            // ancestors からスレッドルートIDを決める
            let root_id = if let Some(first) = ctx.ancestors.first() {
                first.id.clone()
            } else {
                status.id.clone()
            };

            let ctx_text = context::format_conversation_context(&ctx, status);
            let ctx_opt = if ctx_text.is_empty() { None } else { Some(ctx_text) };
            (ctx_opt, root_id)
        }
        Err(e) => {
            eprintln!("Failed to fetch status context: {:?}", e);
            // コンテキスト取れなくても、とりあえずこのステータスIDを thread_key にする
            (None, status.id.clone())
        }
    }
}

async fn load_previous_response_id(
    conv_store: &Arc<ConversationStore>,
    thread_key: &str,
) -> Result<Option<String>> {
    let prev_response_id = conv_store.get_previous_response_id(thread_key).await?;
    if let Some(ref id) = prev_response_id {
        println!("  -> previous_response_id for thread {}: {}", thread_key, id);
    }

    Ok(prev_response_id)
}

fn select_context_for_openai<'a>(
    conversation_context: Option<&'a str>,
    previous_response_id: Option<&String>,
) -> Option<&'a str> {
    if previous_response_id.is_some() {
        // 2回目以降：OpenAI 側の会話状態に任せる
        None
    } else {
        // 初回だけ Mastodon 側の会話ログをブートストラップとして渡す
        conversation_context
    }
}

async fn post_generated_reply(
    client: &reqwest::Client,
    config: &BotConfig,
    status: &crate::mastodon::Status,
    account_acct: &str,
    reply_text: &str,
) {
    // 4-1. Mastodon へ投稿
    if let Err(e) = post_reply(
        client,
        &config.mastodon_base,
        &config.mastodon_access_token,
        status,
        account_acct,
        reply_text,
    )
    .await
    {
        eprintln!("Failed to post reply: {:?}", e);
    }
}

async fn save_response_id(
    conv_store: &Arc<ConversationStore>,
    thread_key: &str,
    response_id: &str,
) {
    // 4-2. このスレッドの last_response_id として保存
    if let Err(e) = conv_store.upsert_last_response_id(thread_key, response_id).await {
        eprintln!("Failed to update last_response_id for thread {}: {:?}", thread_key, e);
    }
}

#[derive(Debug, serde::Deserialize)]
struct StreamEvent {
    event: String,
    payload: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Visibility;
    use std::time::Duration;

    fn test_config() -> BotConfig {
        BotConfig {
            mastodon_base: "https://mastodon.example".to_string(),
            mastodon_access_token: "mastodon-token".to_string(),
            openai_model: "gpt-test".to_string(),
            openai_reply_model: "gpt-test-reply".to_string(),
            openai_api_key: "openai-token".to_string(),
            streaming_base_url: "wss://mastodon.example/api/v1/streaming".to_string(),
            prompts_path: "config/prompts.json".to_string(),
            bot_db_path: ":memory:".to_string(),
            free_toot_interval: Duration::from_secs(3600),
            reply_temperature: 0.7,
            free_toot_temperature: 0.8,
            visibility: Visibility::Unlisted,
            mastodon_char_limit: 500,
            reply_min_interval: Duration::from_millis(0),
            enable_web_search: false,
        }
    }

    fn test_store() -> Arc<ConversationStore> {
        Arc::new(ConversationStore::new(":memory:").unwrap())
    }

    #[tokio::test]
    async fn ignores_non_notification_without_parsing_payload() {
        let client = reqwest::Client::new();
        let config = test_config();
        let store = test_store();
        let text = r#"{"event":"update","payload":"not notification json"}"#;

        handle_ws_text(&client, &config, &store, text).await.unwrap();
    }

    #[tokio::test]
    async fn ignores_non_mention_notifications() {
        let client = reqwest::Client::new();
        let config = test_config();
        let store = test_store();
        let text = r#"{
            "event":"notification",
            "payload":"{\"id\":\"n1\",\"type\":\"favourite\",\"status\":null,\"account\":{\"acct\":\"alice\",\"bot\":false}}"
        }"#;

        handle_ws_text(&client, &config, &store, text).await.unwrap();
    }

    #[tokio::test]
    async fn ignores_bot_mentions_before_requiring_status() {
        let client = reqwest::Client::new();
        let config = test_config();
        let store = test_store();
        let text = r#"{
            "event":"notification",
            "payload":"{\"id\":\"n1\",\"type\":\"mention\",\"status\":null,\"account\":{\"acct\":\"bot\",\"bot\":true}}"
        }"#;

        handle_ws_text(&client, &config, &store, text).await.unwrap();
    }

    #[test]
    fn selects_context_only_without_previous_response() {
        let previous_response_id = "resp_123".to_string();

        assert_eq!(select_context_for_openai(Some("ctx"), None), Some("ctx"));
        assert_eq!(select_context_for_openai(Some("ctx"), Some(&previous_response_id)), None);
        assert_eq!(select_context_for_openai(None, None), None);
    }
}
