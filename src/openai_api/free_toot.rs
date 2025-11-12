use anyhow::Result;
use reqwest::Client;

use crate::openai_api::stream::call_responses;
use crate::openai_api::types::{ChatMessage, ResponsesResult};
use crate::openai_api::prompts::PROMPTS;

pub async fn generate_free_toot(
    client: &Client,
    model: &str,
    api_key: &str,
    temperature: f32,
) -> Result<ResponsesResult> {
    // prompts.json からそのまま取得
    let messages: Vec<ChatMessage> = PROMPTS.free_toot.clone();

    call_responses(
        client,
        model,
        api_key,
        messages,
        Some(temperature),
        Some(256),
        None,
    )
        .await
}
