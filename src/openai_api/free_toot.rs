use anyhow::Result;
use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Asia::Tokyo;
use reqwest::Client;

use crate::config::BotConfig;
use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::stream::{call_responses, CallResponsesArgs};
use crate::openai_api::types::{ChatMessage, Tool};

/// 時間帯に応じて free_toot_*（Vec<ChatMessage>）を選ぶ
fn pick_free_toot_prompt() -> (Vec<ChatMessage>, &'static str) {
    let hour = Utc::now().with_timezone(&Tokyo).hour();
    if (5..=8).contains(&hour) {
        (PROMPTS.free_toot_morning.clone(), "morning")
    } else if (9..=15).contains(&hour) {
        (PROMPTS.free_toot_day.clone(), "day")
    } else if (16..=18).contains(&hour) {
        // 夕方スロット（テンプレ自体は daytime を流用）
        (PROMPTS.free_toot_day.clone(), "evening")
    } else {
        (PROMPTS.free_toot_night.clone(), "night")
    }
}

fn season_label_from_month(month: u32) -> &'static str {
    match month {
        3..=5 => "春",
        6..=8 => "夏",
        9..=11 => "秋",
        _ => "冬", // 12,1,2
    }
}

fn time_label_from_hour(hour: u32) -> &'static str {
    match hour {
        5..=8 => "朝",
        9..=15 => "昼",
        16..=18 => "夕方",
        _ => "夜",
    }
}

/// 実行用の message 配列を組み立て（JST時刻だけ追記）
fn build_messages_for_free_toot() -> (Vec<ChatMessage>, &'static str) {
    let (mut messages, slot) = pick_free_toot_prompt();

    // JSTの現在時刻を取得
    let now_utc: DateTime<Utc> = Utc::now();
    let jst = now_utc.with_timezone(&Tokyo);

    let season = season_label_from_month(jst.month());
    let time_label = time_label_from_hour(jst.hour());

    // 最後の user メッセージを書き換える
    if let Some(user_msg) = messages.iter_mut().rev().find(|m| m.role == "user") {
        // fine-tune に合わせて、季節＋時間帯の指示を埋め込む
        user_msg.content = format!("{season}の{time_label}のような投稿を生成してください。");
    }

    // 現在時刻（JST）を追加（systemメッセージとして）
    messages.push(ChatMessage {
        role: "system".into(),
        content: format!("CurrentTime(JST): {}", jst.to_rfc3339()),
    });

    (messages, slot)
}

pub async fn generate_free_toot(client: &Client, cfg: &BotConfig) -> Result<String> {
    let (messages, slot) = build_messages_for_free_toot();
    println!("[free toot] using {} prompt", slot);

    let model = &cfg.openai_model;
    let model_reply = &cfg.openai_reply_model;
    let api_key = &cfg.openai_api_key;
    let temperature = cfg.free_toot_temperature;

    // time ツールは存在しないので使わない。web_search は preview 名称。
    let mut tools = Vec::new();
    if cfg.enable_web_search {
        tools.push(Tool::WebSearchPreview { search_context_size: None });
    }

    let args = CallResponsesArgs::new(model, model_reply, api_key, messages)
        .temperature(temperature)
        .max_output_tokens(1024)
        .tools(tools);

    let res = call_responses(client, args, false).await?;

    Ok(res.text)
}
