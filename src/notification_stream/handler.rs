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
    let ev: StreamEvent =
        serde_json::from_str(text).context("Failed to parse stream event JSON")?;

    if ev.event != "notification" {
        return Ok(());
    }

    let payload = match ev.payload {
        Some(ref p) => p,
        None => return Ok(()),
    };

    let notif: Notification =
        serde_json::from_str(payload).context("Failed to parse notification payload")?;

    if notif.notif_type != "mention" {
        return Ok(());
    }

    // bot 同士のリプ合戦防止
    if notif.account.bot.unwrap_or(false) {
        println!("Skip mention from bot account @{} (id={})", notif.account.acct, notif.id);
        return Ok(());
    }

    let status = match notif.status {
        Some(ref s) => s,
        None => return Ok(()),
    };

    let plain = strip_html(&status.content);
    println!("(stream) Mention from @{}: {}", notif.account.acct, plain);

    // 会話コンテキスト取得
    let (conversation_context, thread_key) = match fetch_status_context(
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
    };

    // 2. SQLite から previous_response_id を取得
    let prev_response_id = conv_store.get_previous_response_id(&thread_key).await?;
    if let Some(ref id) = prev_response_id {
        println!("  -> previous_response_id for thread {}: {}", thread_key, id);
    }

    // ここで「初回だけ context を渡す」ように分岐
    let context_for_openai: Option<&str> = if prev_response_id.is_some() {
        // 2回目以降：OpenAI 側の会話状態に任せる
        None
    } else {
        // 初回だけ Mastodon 側の会話ログをブートストラップとして渡す
        conversation_context.as_deref()
    };

    // 3. レートリミット
    wait_for_rate_limit(config.reply_min_interval.as_millis() as u64).await;

    // 4. OpenAI へ問い合わせ（previous_response_id を渡す）
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

            // 4-1. Mastodon へ投稿
            if let Err(e) = post_reply(
                client,
                &config.mastodon_base,
                &config.mastodon_access_token,
                status,
                &notif.account.acct,
                &reply_result.text,
            )
            .await
            {
                eprintln!("Failed to post reply: {:?}", e);
            }

            // 4-2. このスレッドの last_response_id として保存
            if let Err(e) =
                conv_store.upsert_last_response_id(&thread_key, &reply_result.response_id).await
            {
                eprintln!("Failed to update last_response_id for thread {}: {:?}", thread_key, e);
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
