use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

use crate::openai_api::types::{ChatMessage, ResponsesRequest, ResponsesResult, Tool};

/// `call_responses` に渡す引数まとめ
pub struct CallResponsesArgs<'a> {
    pub model: &'a str,
    pub model_reply: &'a str,
    pub api_key: &'a str,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub previous_response_id: Option<String>,
    pub tools: Option<Vec<Tool>>,
}

impl<'a> CallResponsesArgs<'a> {
    pub fn new(model: &'a str, model_reply: &'a str, api_key: &'a str, messages: Vec<ChatMessage>) -> Self {
        Self {
            model,
            model_reply,
            api_key,
            messages,
            temperature: None,
            max_output_tokens: None,
            previous_response_id: None,
            tools: None,
        }
    }
    pub fn temperature(mut self, t: f32) -> Self { self.temperature = Some(t); self }
    pub fn max_output_tokens(mut self, n: u32) -> Self { self.max_output_tokens = Some(n); self }
    pub fn previous_response_id<S: Into<String>>(mut self, id: S) -> Self {
        self.previous_response_id = Some(id.into()); self
    }
    pub fn tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = if tools.is_empty() { None } else { Some(tools) }; self
    }
}

/// `{"type":"output_text","text":"..."}` を優先的に抽出
fn extract_output_text(v: &Value, out: &mut String) {
    match v {
        Value::Object(map) => {
            // ← ネスト if を合体（if-let ガード）
            if let (Some(Value::String(ty)), Some(Value::String(t))) =
                (map.get("type"), map.get("text"))
                && ty == "output_text"
            {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(t);
            }

            // キー未使用なので values() でスッキリ
            for vv in map.values() {
                extract_output_text(vv, out);
            }
        }
        Value::Array(arr) => {
            for vv in arr {
                extract_output_text(vv, out);
            }
        }
        _ => {}
    }
}

/// OpenAI Responses API 呼び出し（JSONをValueで受けて安全抽出）
pub async fn call_responses(client: &Client, args: CallResponsesArgs<'_>, is_reply: bool) -> Result<ResponsesResult> {
    let (instructions, input) = split_messages_for_responses(args.messages);

    let model = if is_reply { args.model_reply.to_string() } else { args.model.to_string() };

    let temperature =
        if model.contains("gpt-5") { None } else { args.temperature };

    let req_body = ResponsesRequest {
        model,
        input,
        instructions,
        temperature,
        max_output_tokens: args.max_output_tokens,
        previous_response_id: args.previous_response_id,
        tools: args.tools,
    };

    let resp = client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(args.api_key)
        .json(&req_body)
        .send()
        .await?;

    let status_code = resp.status();
    let raw = resp.text().await?;

    if !status_code.is_success() {
        return Err(anyhow!("OpenAI error {}: {}", status_code, raw));
    }

    let v: Value = serde_json::from_str(&raw)
        .map_err(|e| anyhow!("error decoding response body: {}\nraw: {}", e, raw))?;

    let id = v.get("id").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let status = v.get("status").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let mut text = String::new();

    if let Some(output) = v.get("output") {
        extract_output_text(output, &mut text);
    }
    if text.is_empty() {
        // ぜんぜん拾えなかった場合は空文字のまま返し、呼び出し側でリカバリ
        // （ここで raw を返して Mastodon に貼らない）
    }

    Ok(ResponsesResult { id, text, status: Some(status) })
}

fn split_messages_for_responses(
    messages: Vec<ChatMessage>,
) -> (Option<String>, Vec<ChatMessage>) {
    let mut system_chunks = Vec::new();
    let mut input_messages = Vec::new();

    for msg in messages {
        // role の型が String ならこんな感じ
        if msg.role == "system" {
            system_chunks.push(msg.content.clone());
        } else {
            input_messages.push(msg);
        }
    }

    let instructions = if system_chunks.is_empty() {
        None
    } else {
        // system メッセージが複数あってもまとめて 1 本の instructions にする
        Some(system_chunks.join("\n\n"))
    };

    (instructions, input_messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_single_output_text() {
        let v = json!({
            "output": [
                {"type":"output_text","text":"hello"}
            ]
        });
        let mut out = String::new();
        extract_output_text(&v, &mut out);
        assert_eq!(out, "hello");
    }

    #[test]
    fn extract_multiple_output_text_joined_with_newlines() {
        let v = json!({
            "output": [
                {"type":"output_text","text":"line1"},
                {"type":"other","text":"ignored"},
                {"type":"output_text","text":"line2"}
            ]
        });
        let mut out = String::new();
        extract_output_text(&v, &mut out);
        assert_eq!(out, "line1\nline2");
    }

    #[test]
    fn ignore_when_no_text() {
        let v = json!({ "output": [{"type":"other","text":"x"}] });
        let mut out = String::new();
        extract_output_text(&v, &mut out);
        assert!(out.is_empty());
    }
}
