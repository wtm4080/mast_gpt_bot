use anyhow::Result;
use reqwest::Client;
use chrono::{Local, Timelike};

use crate::openai_api::stream::call_responses;
use crate::openai_api::types::ChatMessage;
use crate::openai_api::prompts::PROMPTS;

/// 時間帯に応じて、どの free_toot プロンプトを使うか選ぶ
fn select_free_toot_messages() -> (Vec<ChatMessage>, &'static str) {
    let now = Local::now();
    let hour = now.hour();

    // ざっくりこんな感じで分ける:
    // 5〜11時: 朝
    // 11〜18時: 昼
    // 18〜翌2時: 夜
    // それ以外（2〜5時）：いちおう夜扱いにしちゃう
    if (5..11).contains(&hour) {
        (PROMPTS.free_toot_morning.clone(), "morning")
    } else if (11..18).contains(&hour) {
        (PROMPTS.free_toot_day.clone(), "day")
    } else {
        (PROMPTS.free_toot_night.clone(), "night")
    }
}

pub async fn generate_free_toot(
    client: &Client,
    model: &str,
    api_key: &str,
    temperature: f32,
) -> Result<String> {
    let (messages, slot) = select_free_toot_messages();
    println!("[free toot] using {} prompts", slot);

    let res = call_responses(
        client,
        model,
        api_key,
        messages,
        Some(temperature),
        Some(256),
        None, // previous_response_id は自由トゥートには使わない
    )
        .await?;

    Ok(res.text)
}
