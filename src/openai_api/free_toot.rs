use anyhow::Result;
use chrono::{Local, Timelike};
use reqwest::Client;

use crate::config::BotConfig;
use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::stream::call_responses;
use crate::openai_api::types::{ChatMessage, Tool};

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

pub async fn generate_free_toot(client: &Client, cfg: &BotConfig) -> Result<String> {
    let (messages, slot) = select_free_toot_messages();
    println!("[free toot] using {} prompts", slot);

    let model = &cfg.openai_model;
    let api_key = &cfg.openai_api_key;
    let temperature = cfg.free_toot_temperature;

    let mut tools = Vec::new();
    if cfg.enable_web_search {
        tools.push(Tool::WebSearch);
    }
    if cfg.enable_time_now {
        tools.push(Tool::Time);
    }

    let res = call_responses(
        client,
        model,
        api_key,
        messages,
        Some(temperature),
        Some(256),
        None, // previous_response_id は自由トゥートでは未使用
        if tools.is_empty() { None } else { Some(tools) },
    )
    .await?;

    Ok(res.text)
}
