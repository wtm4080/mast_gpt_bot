///! OpenAI API まわり（返信生成＆自由トゥート生成）

use anyhow::{Context, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageResp,
}

#[derive(Debug, Deserialize)]
struct ChatMessageResp {
    content: String,
}

const OPENAI_CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

/// メンションに対する返信を生成
pub async fn generate_reply(
    client: &Client,
    model: &str,
    api_key: &str,
    user_text: &str,
) -> Result<String> {
    let req_body = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: "あなたは Mastodon のタイムラインでゆるく喋る日本語話者です。丁寧すぎない口調で、相手を安心させる感じで返信してください。ただし失礼な言い方や攻撃的な表現はしないでください。"
                    .into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_text.to_string(),
            },
        ],
    };

    let resp = client
        .post(OPENAI_CHAT_URL)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&req_body)
        .send()
        .await
        .context("OpenAI API request failed (reply)")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI error {}: {}", status, text);
    }

    let json: ChatResponse = resp.json().await.context("parse OpenAI reply")?;
    let content = json
        .choices
        .get(0)
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_else(|| "（うまく返事を考えられなかった…）".to_string());

    Ok(content)
}

/// 1時間に1回の「自由トゥート」を生成
pub async fn generate_free_toot(
    client: &Client,
    model: &str,
    api_key: &str,
) -> Result<String> {
    let req_body = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: "あなたは Mastodon に投稿する日本語話者です。タイムラインにそのまま流せるような、短めの自然なつぶやきを生成してください。攻撃的・不適切な表現は使わず、1〜2文程度に収めてください。"
                    .into(),
            },
            ChatMessage {
                role: "user".into(),
                content: "今の気分で、自由につぶやいてください。".into(),
            },
        ],
    };

    let resp = client
        .post(OPENAI_CHAT_URL)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&req_body)
        .send()
        .await
        .context("OpenAI API request failed (free toot)")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI error {}: {}", status, text);
    }

    let json: ChatResponse = resp.json().await.context("parse OpenAI free toot")?;
    let content = json
        .choices
        .get(0)
        .map(|c| c.message.content.trim().to_string())
        .unwrap_or_else(|| "（なんてつぶやけばいいか分からない…）".to_string());

    Ok(content)
}
