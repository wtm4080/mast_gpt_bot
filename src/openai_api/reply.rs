use anyhow::Result;
use reqwest::Client;

use crate::openai_api::stream::call_responses;
use crate::openai_api::types::{ChatMessage, ResponsesResult};
use crate::openai_api::prompts::PROMPTS;

pub struct ReplyResult {
    pub text: String,
    pub response_id: String,
}

pub async fn generate_reply(
    client: &Client,
    model: &str,
    api_key: &str,
    user_text: &str,
    conversation_context: Option<&str>,
    temperature: f32,
    previous_response_id: Option<String>,
) -> Result<ReplyResult> {
    // どっちのテンプレを使うかだけ分岐
    let mut messages: Vec<ChatMessage> = if conversation_context.is_some() {
        PROMPTS.reply_with_context.clone()
    } else {
        PROMPTS.reply_without_context.clone()
    };

    let ctx_str = conversation_context.unwrap_or("");

    // user メッセージにだけプレースホルダ差し替え
    for msg in &mut messages {
        if msg.role == "user" {
            msg.content = msg
                .content
                .replace("{{USER_TEXT}}", user_text)
                .replace("{{CONTEXT}}", ctx_str); // ← もう使わなくても OK（入ってなければ no-op）
        }
    }

    if !messages.iter().any(|m| m.role == "system") {
        messages.insert(
            0,
            ChatMessage {
                role: "system".into(),
                content: "あなたは Mastodon のタイムラインでゆるく喋る日本語話者です。丁寧すぎない口調で、相手を安心させる感じで返信してください。ただし失礼な言い方や攻撃的な表現はしないでください。"
                    .into(),
            },
        );
    }

    let res: ResponsesResult = call_responses(
        client,
        model,
        api_key,
        messages,
        Some(temperature),
        Some(256),
        previous_response_id,
    )
        .await?;

    Ok(ReplyResult {
        text: res.text,
        response_id: res.id,
    })
}
