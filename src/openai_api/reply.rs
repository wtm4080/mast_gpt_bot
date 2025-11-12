use anyhow::Result;
use chrono::{DateTime, Utc};
use chrono_tz::Asia::Tokyo;
use reqwest::Client;

use crate::config::BotConfig;
use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::stream::call_responses;
use crate::openai_api::types::{ChatMessage, ResponsesResult, Tool};

pub struct ReplyResult {
    pub text: String,
    pub response_id: String,
}

fn now_tokyo_rfc3339() -> String {
    let now_utc: DateTime<Utc> = Utc::now();
    let jst = now_utc.with_timezone(&Tokyo);
    jst.to_rfc3339()
}

/// コンテキストの有無で reply_*（Vec<ChatMessage>）を選び、時刻や本文を積む
fn build_messages(user_text: &str, conversation_context: Option<&str>) -> Vec<ChatMessage> {
    // ベースのプロンプト（Vec<ChatMessage>）
    let mut messages: Vec<ChatMessage> = if conversation_context.is_some() {
        PROMPTS.reply_with_context.clone()
    } else {
        PROMPTS.reply_without_context.clone()
    };

    // 現在時刻（JST）を system で追加
    messages.push(ChatMessage {
        role: "system".into(),
        content: format!("CurrentTime(JST): {}", now_tokyo_rfc3339()),
    });

    // （必要なら）会話コンテキストをユーザーメッセージとして添付
    if let Some(ctx) = conversation_context {
        messages.push(ChatMessage {
            role: "user".into(),
            content: format!("[context]\n{}", ctx),
        });
    }

    // ユーザーの入力本文
    messages.push(ChatMessage {
        role: "user".into(),
        content: user_text.to_string(),
    });

    messages
}

pub async fn generate_reply(
    client: &Client,
    cfg: &BotConfig,
    user_text: &str,
    conversation_context: Option<&str>,
    previous_response_id: Option<String>,
) -> Result<ReplyResult> {
    let model = &cfg.openai_model;
    let api_key = &cfg.openai_api_key;
    let temperature = cfg.reply_temperature;

    let messages: Vec<ChatMessage> = build_messages(user_text, conversation_context);

    let mut tools = Vec::new();
    if cfg.enable_web_search {
        tools.push(Tool::WebSearchPreview);
    }

    let res: ResponsesResult = call_responses(
        client,
        model,
        api_key,
        messages,
        Some(temperature),
        Some(256),
        previous_response_id,
        if tools.is_empty() { None } else { Some(tools) },
    )
        .await?;

    Ok(ReplyResult { text: res.text, response_id: res.id })
}
