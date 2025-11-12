use crate::openai_api::types::{ChatMessage, ChatRequest, ChatStreamResponse};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;

const OPENAI_CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

pub async fn chat_stream(
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
    let mut pending = String::new(); // チャンク横断バッファ

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed to read streaming chunk")?;
        let text = String::from_utf8_lossy(&chunk);

        pending.push_str(&text);

        // pending 内に '\n' がある間、1行ずつ処理
        loop {
            if let Some(pos) = pending.find('\n') {
                let line = pending[..pos].to_string();
                pending = pending[pos + 1..].to_string();

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if !line.starts_with("data: ") {
                    continue;
                }

                let data = &line["data: ".len()..];

                if data == "[DONE]" {
                    return Ok(output.trim().to_string());
                }

                if data.is_empty() {
                    continue;
                }

                let parsed: ChatStreamResponse = match serde_json::from_str(data) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Failed to parse stream JSON: {} | raw: {}", e, data);
                        continue;
                    }
                };

                if let Some(choice) = parsed.choices.get(0) {
                    if let Some(delta) = &choice.delta.content {
                        output.push_str(delta);
                    }
                }
            } else {
                break;
            }
        }
    }

    Ok(output.trim().to_string())
}
