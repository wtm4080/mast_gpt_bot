use anyhow::Result;
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Asia::Tokyo;
use reqwest::Client;

use crate::config::BotConfig;
use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::stream::call_responses;
use crate::openai_api::types::{ChatMessage, Tool};

fn now_tokyo_rfc3339() -> String {
    let now_utc: DateTime<Utc> = Utc::now();
    let jst = now_utc.with_timezone(&Tokyo);
    jst.to_rfc3339()
}

/// 時間帯に応じて free_toot_*（Vec<ChatMessage>）を選ぶ
fn pick_free_toot_prompt() -> (Vec<ChatMessage>, &'static str) {
    let hour = Utc::now().with_timezone(&Tokyo).hour();
    if (5..=10).contains(&hour) {
        (PROMPTS.free_toot_morning.clone(), "morning")
    } else if (11..=18).contains(&hour) {
        (PROMPTS.free_toot_day.clone(), "day")
    } else {
        (PROMPTS.free_toot_night.clone(), "night")
    }
}

/// 実行用の message 配列を組み立て（JST時刻だけ追記）
fn build_messages_for_free_toot() -> (Vec<ChatMessage>, &'static str) {
    let (mut messages, slot) = pick_free_toot_prompt();

    // 現在時刻（JST）を追加（systemメッセージとして）
    messages.push(ChatMessage {
        role: "system".into(),
        content: format!("CurrentTime(JST): {}", now_tokyo_rfc3339()),
    });

    (messages, slot)
}

pub async fn generate_free_toot(client: &Client, cfg: &BotConfig) -> Result<String> {
    let (messages, slot) = build_messages_for_free_toot();
    println!("[free toot] using {} prompt", slot);

    let model = &cfg.openai_model;
    let api_key = &cfg.openai_api_key;
    let temperature = cfg.free_toot_temperature;

    // time ツールは存在しないので使わない。web_search は preview 名称。
    let mut tools = Vec::new();
    if cfg.enable_web_search {
        tools.push(Tool::WebSearchPreview);
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
