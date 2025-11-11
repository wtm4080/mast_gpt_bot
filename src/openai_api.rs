///! OpenAI API まわり（返信生成＆自由トゥート生成）

// src/openai_api.rs
use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENAI_CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatStreamResponse {
    choices: Vec<ChatStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatStreamChoice {
    delta: ChatStreamDelta,
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatStreamDelta {
    content: Option<String>,
}

/// 汎用: streaming で ChatCompletion を呼び出し、最終テキストを返す
async fn chat_stream(
    client: &Client,
    model: &str,
    api_key: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
) -> Result<String> {
    let req_body = ChatRequest {
        model: model.to_string(),
        messages,
        temperature,
        stream: Some(true),
    };

    let resp = client
        .post(OPENAI_CHAT_URL)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .header("Accept", "text/event-stream")
        .json(&req_body)
        .send()
        .await
        .context("OpenAI API request failed (stream)")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI error {}: {}", status, text);
    }

    let mut stream = resp.bytes_stream();
    let mut output = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to read streaming chunk")?;
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if !line.starts_with("data: ") {
                continue;
            }

            let data = &line["data: ".len()..];

            if data == "[DONE]" {
                // 完了
                return Ok(output.trim().to_string());
            }

            let parsed: ChatStreamResponse =
                serde_json::from_str(data).context("failed to parse stream JSON")?;
            if let Some(choice) = parsed.choices.get(0) {
                if let Some(delta) = &choice.delta.content {
                    output.push_str(delta);
                }
            }
        }
    }

    Ok(output.trim().to_string())
}

/// メンションに対する返信を生成（streaming）
pub async fn generate_reply(
    client: &Client,
    model: &str,
    api_key: &str,
    user_text: &str,
    conversation_context: Option<&str>,   // ★ 追加
) -> Result<String> {
    let system_msg = ChatMessage {
        role: "system".into(),
        content: "あなたは Mastodon のタイムラインでゆるく喋る日本語話者です。丁寧すぎない口調で、相手を安心させる感じで返信してください。ただし失礼な言い方や攻撃的な表現はしないでください。"
            .into(),
    };

    let user_msg = if let Some(ctx) = conversation_context {
        ChatMessage {
            role: "user".into(),
            content: format!(
                concat!(
                "以下は、これまでの会話の流れです（古い順）。一番下が相手の最新の投稿です:\n",
                "{}\n\n",
                "この会話の流れを踏まえて、相手の最新の投稿に対する返信として、",
                "Mastodon に投稿できる短めのメッセージを書いてください。\n",
                "同じ質問に対しても、できるだけ毎回少し表現を変えてください。\n",
                "必要があれば、もう1〜2文だけ軽く補足してもOKです。",
                ),
                ctx
            ),
        }
    } else {
        // 文脈が取れなかった場合は、これまでどおり単発の投稿として扱う
        ChatMessage {
            role: "user".into(),
            content: format!(
                concat!(
                "自由につぶやいてください。相手の投稿への返信として一言を書いてください。\n",
                "同じ質問に対しても、できるだけ毎回少し表現を変えてください。\n",
                "必要があれば、もう1〜2文だけ軽く説明を足してもOKです。\n",
                "\n",
                "相手の投稿: {}\n",
                ),
                user_text
            ),
        }
    };

    let messages = vec![system_msg, user_msg];

    // 会話返信はちょい変化欲しいので temperature 0.6くらい
    chat_stream(client, model, api_key, messages, Some(0.6)).await
}

/// 1時間に1回の「自由トゥート」を生成（streaming）
pub async fn generate_free_toot(
    client: &Client,
    model: &str,
    api_key: &str,
) -> Result<String> {
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "あなたは Mastodon に投稿する日本語話者です。タイムラインにそのまま流せるような、短めの自然なつぶやきを生成してください。攻撃的・不適切な表現は使わず、1〜2文程度に収めてください。"
                .into(),
        },
        ChatMessage {
            role: "user".into(),
            content: "今の気分で、自由につぶやいてください。".into(),
        },
    ];

    chat_stream(client, model, api_key, messages, Some(0.8)).await
}
