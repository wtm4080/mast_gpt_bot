use anyhow::Result;
use chrono::{DateTime, Utc};
use chrono_tz::Asia::Tokyo;
use regex::Regex;
use reqwest::Client;

use crate::config::BotConfig;
use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::stream::{call_responses, CallResponsesArgs};
use crate::openai_api::types::{ChatMessage, ResponsesResult, Tool};

pub struct ReplyResult {
    pub text: String,
    pub response_id: String,
}

fn now_tokyo_rfc3339() -> String {
    let now_utc: DateTime<Utc> = Utc::now();
    let jst = now_utc.with_timezone(&Tokyo);
    jst.to_rfc3339()
}

fn should_force_search(user_text: &str) -> bool {
    let jp = Regex::new(r"(リリースノート|変更点|変更履歴|ハイライト|新機能|何が(新しい|変わった)|教えて)").unwrap();
    let en = Regex::new(r"(release\s*notes?|changelog|what'?s\s*new|highlights?|patch\s*notes?)").unwrap();
    let ver = Regex::new(r"\b\d+\.\d+(\.\d+)?\b").unwrap();
    jp.is_match(user_text) || en.is_match(user_text) || ver.is_match(user_text)
}

fn base_prompt_for_reply(conversation_context: Option<&str>) -> Vec<ChatMessage> {
    if conversation_context.is_some() {
        PROMPTS.reply_with_context.clone()
    } else {
        PROMPTS.reply_without_context.clone()
    }
}

// 短文化＆プレーン＆推測禁止の強い制約
fn search_mandate_instruction() -> String {
    [
        // いつ検索するか
        "When asked about versions/release notes/highlights:",
        "• You MUST use web_search to fetch official sources.",
        // 出力フォーマット（超厳しめ）
        "• Output: 2 bullets max, plain text only.",
        "• Each bullet ≤ 70 Japanese chars.",
        "• Include the exact version and a YYYY-MM-DD (JST) date.",
        "• Add one source domain in parentheses, e.g., (blog.rust-lang.org).",
        // 推測禁止＆将来の扱い
        "• Do NOT speculate about future releases.",
        "• If a future date isn't confirmed by official sources, say “未確定”.",
        // リンク/Markdown 禁止
        "• NO URLs and NO markdown. Do not output '[' ']' '(' within URLs.",
        // 全体長
        "• Keep total length ≤ 180 Japanese chars.",
        // ツール呼び出し回数
        "• Perform at most one search call.",
    ].join(" ")
}

fn build_messages(user_text: &str, conversation_context: Option<&str>, force_search: bool) -> Vec<ChatMessage> {
    let mut msgs: Vec<ChatMessage> = base_prompt_for_reply(conversation_context);

    // 現在時刻（JST）
    msgs.push(ChatMessage {
        role: "system".into(),
        content: format!("CurrentTime(JST): {}", now_tokyo_rfc3339()),
    });

    if force_search {
        // ここで強い制約を合流
        msgs.push(ChatMessage {
            role: "system".into(),
            content: search_mandate_instruction(),
        });

        // 追加: “パッチ版は要点を1-2点のみ” の明示
        msgs.push(ChatMessage {
            role: "system".into(),
            content: "For patch releases (e.g., 1.91.1), summarize only 1–2 key fixes.".parse().unwrap(),
        });
    }

    if let Some(ctx) = conversation_context {
        msgs.push(ChatMessage { role: "user".into(), content: format!("[context]\n{}", ctx) });
    }

    msgs.push(ChatMessage { role: "user".into(), content: user_text.to_string() });
    msgs
}

pub async fn generate_reply(
    client: &Client,
    cfg: &BotConfig,
    user_text: &str,
    conversation_context: Option<&str>,
    previous_response_id: Option<String>,
) -> Result<ReplyResult> {
    let force_search = should_force_search(user_text);

    let model = &cfg.openai_model;
    let api_key = &cfg.openai_api_key;

    // さらに短め設定（出力が途切れないよう max を控えめに）
    let messages: Vec<ChatMessage> = build_messages(user_text, conversation_context, force_search);

    let mut tools = Vec::new();
    if cfg.enable_web_search || force_search {
        // 検索は軽めのコンテキストで十分
        tools.push(Tool::WebSearchPreview { search_context_size: Some("low".into()) });
    }

    let mut builder = CallResponsesArgs::new(model, api_key, messages)
        .temperature(0.2_f32)
        .max_output_tokens(140);

    if let Some(prev) = previous_response_id {
        builder = builder.previous_response_id(prev);
    }
    if !tools.is_empty() {
        builder = builder.tools(tools);
    }

    let mut res: ResponsesResult = call_responses(client, builder).await?;

    // 再試行（もっと短く＆“未確定”の指示を再強調）
    if res.text.trim().is_empty() || res.status.as_deref() == Some("incomplete") {
        let mut retry_msgs = base_prompt_for_reply(conversation_context);
        retry_msgs.push(ChatMessage {
            role: "system".into(),
            content: "2 bullets max. ≤ 60 Japanese chars each. Plain text. No URLs. Unconfirmed future dates → “未確定”.".parse()?,
        });
        retry_msgs.push(ChatMessage { role: "system".into(), content: format!("CurrentTime(JST): {}", now_tokyo_rfc3339()) });
        if let Some(ctx) = conversation_context {
            retry_msgs.push(ChatMessage { role: "user".into(), content: format!("[context]\n{}", ctx) });
        }
        retry_msgs.push(ChatMessage { role: "user".into(), content: user_text.to_string() });

        let mut retry_tools = Vec::new();
        if cfg.enable_web_search || force_search {
            retry_tools.push(Tool::WebSearchPreview { search_context_size: Some("low".into()) });
        }

        let mut retry_builder = CallResponsesArgs::new(model, api_key, retry_msgs)
            .temperature(0.1_f32)
            .max_output_tokens(120);
        if !retry_tools.is_empty() {
            retry_builder = retry_builder.tools(retry_tools);
        }

        let retry_res: ResponsesResult = call_responses(client, retry_builder).await?;
        if !retry_res.text.trim().is_empty() {
            res = retry_res;
        }
    }

    // 最後の保険：JSONを潰す
    let clean = res.text.trim();
    let final_text = if clean.starts_with('{') || clean.starts_with('[') {
        "短く要点＋出典ドメインでまとめられなかったみたい。もう一度聞いて！".to_string()
    } else {
        clean.to_string()
    };

    Ok(ReplyResult { text: final_text, response_id: res.id })
}
