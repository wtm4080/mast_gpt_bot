use anyhow::Result;
use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Asia::Tokyo;
use reqwest::Client;

use crate::config::BotConfig;
use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::stream::{CallResponsesArgs, call_responses};
use crate::openai_api::types::{ChatMessage, Tool};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FreeTootSlot {
    Morning,
    Day,
    Evening,
    Night,
}

impl FreeTootSlot {
    fn as_log_label(self) -> &'static str {
        match self {
            FreeTootSlot::Morning => "morning",
            FreeTootSlot::Day => "day",
            FreeTootSlot::Evening => "evening",
            FreeTootSlot::Night => "night",
        }
    }
}

fn pick_free_toot_prompt_for_slot(slot: FreeTootSlot) -> (Vec<ChatMessage>, &'static str) {
    match slot {
        FreeTootSlot::Morning => (PROMPTS.free_toot_morning.clone(), slot.as_log_label()),
        FreeTootSlot::Day => (PROMPTS.free_toot_day.clone(), slot.as_log_label()),
        FreeTootSlot::Evening => {
            // 夕方スロット（テンプレ自体は daytime を流用）
            (PROMPTS.free_toot_day.clone(), slot.as_log_label())
        }
        FreeTootSlot::Night => (PROMPTS.free_toot_night.clone(), slot.as_log_label()),
    }
}

fn free_toot_slot_from_hour(hour: u32) -> FreeTootSlot {
    match hour {
        5..=8 => FreeTootSlot::Morning,
        9..=15 => FreeTootSlot::Day,
        16..=18 => FreeTootSlot::Evening,
        _ => FreeTootSlot::Night,
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

fn current_jst() -> DateTime<chrono_tz::Tz> {
    Utc::now().with_timezone(&Tokyo)
}

fn apply_free_toot_user_prompt(messages: &mut [ChatMessage], season: &str, time_label: &str) {
    // 最後の user メッセージを書き換える
    if let Some(user_msg) = messages.iter_mut().rev().find(|m| m.role == "user") {
        // fine-tune に合わせて、季節＋時間帯の指示を埋め込む
        user_msg.content = format!("{season}の{time_label}のような投稿を生成してください。");
    }
}

/// 実行用の message 配列を組み立て（JST時刻だけ追記）
fn build_messages_for_free_toot() -> (Vec<ChatMessage>, &'static str) {
    build_messages_for_free_toot_at(current_jst())
}

fn build_messages_for_free_toot_at(
    jst: DateTime<chrono_tz::Tz>,
) -> (Vec<ChatMessage>, &'static str) {
    let (mut messages, slot) = pick_free_toot_prompt_for_slot(free_toot_slot_from_hour(jst.hour()));
    let season = season_label_from_month(jst.month());
    let time_label = time_label_from_hour(jst.hour());
    apply_free_toot_user_prompt(&mut messages, season, time_label);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { role: role.to_string(), content: content.to_string() }
    }

    #[test]
    fn free_toot_slot_boundaries_match_existing_hour_ranges() {
        assert_eq!(free_toot_slot_from_hour(4), FreeTootSlot::Night);
        assert_eq!(free_toot_slot_from_hour(5), FreeTootSlot::Morning);
        assert_eq!(free_toot_slot_from_hour(8), FreeTootSlot::Morning);
        assert_eq!(free_toot_slot_from_hour(9), FreeTootSlot::Day);
        assert_eq!(free_toot_slot_from_hour(15), FreeTootSlot::Day);
        assert_eq!(free_toot_slot_from_hour(16), FreeTootSlot::Evening);
        assert_eq!(free_toot_slot_from_hour(18), FreeTootSlot::Evening);
        assert_eq!(free_toot_slot_from_hour(19), FreeTootSlot::Night);
    }

    #[test]
    fn season_labels_match_existing_month_ranges() {
        assert_eq!(season_label_from_month(1), "冬");
        assert_eq!(season_label_from_month(3), "春");
        assert_eq!(season_label_from_month(6), "夏");
        assert_eq!(season_label_from_month(9), "秋");
        assert_eq!(season_label_from_month(12), "冬");
    }

    #[test]
    fn time_labels_match_existing_hour_ranges() {
        assert_eq!(time_label_from_hour(4), "夜");
        assert_eq!(time_label_from_hour(5), "朝");
        assert_eq!(time_label_from_hour(9), "昼");
        assert_eq!(time_label_from_hour(16), "夕方");
        assert_eq!(time_label_from_hour(19), "夜");
    }

    #[test]
    fn apply_free_toot_user_prompt_rewrites_last_user_message_only() {
        let mut messages =
            vec![message("user", "first"), message("system", "system"), message("user", "last")];

        apply_free_toot_user_prompt(&mut messages, "春", "朝");

        assert_eq!(messages[0].content, "first");
        assert_eq!(messages[1].content, "system");
        assert_eq!(messages[2].content, "春の朝のような投稿を生成してください。");
    }
}
