use anyhow::Result;
use reqwest::Client;

use crate::openai_api::stream::chat_stream;
use crate::openai_api::types::ChatMessage;
use crate::openai_api::prompts::PROMPTS;

pub async fn generate_free_toot(
    client: &Client,
    model: &str,
    api_key: &str,
) -> Result<String> {
    // prompts.json からそのまま取得して clone
    let messages: Vec<ChatMessage> = PROMPTS.free_toot.clone();

    // 自由トゥートはちょっと遊ばせて 0.7 くらい
    chat_stream(client, model, api_key, messages, Some(0.7)).await
}
