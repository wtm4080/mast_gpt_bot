use crate::openai_api::types::{
    ChatMessage, ResponsesRequest, ResponsesResponse, ResponsesResult,
};

use anyhow::{Context, Result};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;

const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";

/// Responses API を叩いて、テキストと response.id を返す
pub async fn call_responses(
    client: &Client,
    model: &str,
    api_key: &str,
    messages: Vec<ChatMessage>,
    temperature: Option<f32>,
    max_output_tokens: Option<u32>,
    previous_response_id: Option<String>,
) -> Result<ResponsesResult> {
    let req_body = ResponsesRequest {
        model: model.to_string(),
        input: messages,
        temperature,
        max_output_tokens,
        previous_response_id,
    };

    let resp = client
        .post(OPENAI_RESPONSES_URL)
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&req_body)
        .send()
        .await
        .context("OpenAI Responses API request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI error {}: {}", status, text);
    }

    let body: ResponsesResponse = resp
        .json()
        .await
        .context("Failed to parse Responses API JSON")?;

    let mut out = String::new();
    for item in body.output {
        for c in item.content {
            if c.content_type == "output_text" {
                if let Some(t) = c.text {
                    out.push_str(&t);
                }
            }
        }
    }

    Ok(ResponsesResult {
        id: body.id,
        text: out.trim().to_string(),
    })
}
