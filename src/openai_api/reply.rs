use anyhow::Result;
use reqwest::Client;

use crate::openai_api::stream::call_responses;
use crate::openai_api::types::ChatMessage;
use crate::openai_api::prompts::PROMPTS;

pub async fn generate_reply(
    client: &Client,
    model: &str,
    api_key: &str,
    user_text: &str,
    conversation_context: Option<&str>,
    temperature: f32, // BotConfig から渡してるやつ
) -> Result<String> {
    // prompts.json からベースのメッセージ配列を取得
    let mut messages: Vec<ChatMessage> = if let Some(ctx) = conversation_context {
        // 会話コンテキストあり
        let mut base = PROMPTS.reply_with_context.clone();

        for msg in &mut base {
            if msg.role == "user" {
                msg.content = msg.content.replace("{{CONTEXT}}", ctx);
            }
        }

        base
    } else {
        // 文脈なし（単発メンション）の場合
        let mut base = PROMPTS.reply_without_context.clone();

        for msg in &mut base {
            if msg.role == "user" {
                msg.content = msg.content.replace("{{USER_TEXT}}", user_text);
            }
        }

        base
    };

    // 念のため system メッセージ保険（通常は prompts.json に入ってる前提）
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

    // Responses API を呼ぶ（max_output_tokens は適当に 256 くらい）
    call_responses(
        client,
        model,
        api_key,
        messages,
        Some(temperature),
        Some(256),
    )
        .await
}
