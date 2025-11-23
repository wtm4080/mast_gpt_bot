use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::types::ChatMessage;

use super::time::now_tokyo_rfc3339;

pub(super) fn build_initial_messages(
    user_text: &str,
    conversation_context: Option<&str>,
    force_search: bool,
) -> Vec<ChatMessage> {
    let base = base_prompt_for_reply(conversation_context);
    let (mut msgs, had_user_placeholder, had_context_placeholder) =
        apply_placeholders(base, user_text, conversation_context);

    msgs.push(ChatMessage {
        role: "system".into(),
        content: format!("CurrentTime(JST): {}", now_tokyo_rfc3339()),
    });

    msgs.push(ChatMessage {
        role: "system".into(),
        content: "ユーザーの発言をそのまま繰り返すだけの返答は禁止です。必ず質問や発言の内容に答え、そのうえで必要なら短くボケや\
相槌を添えてください。質問文を引用するときは、その後に必ずあなたの考えを書くこと。".into(),
    });

    if force_search {
        msgs.push(ChatMessage { role: "system".into(), content: search_mandate_instruction() });

        msgs.push(ChatMessage {
            role: "system".into(),
            content: "For patch releases (e.g., 1.91.1), summarize only 1–2 key fixes.".into(),
        });
    }

    if let Some(ctx) = conversation_context {
        if !had_context_placeholder {
            msgs.push(ChatMessage {
                role: "system".into(),
                content: format!("[context]\n{}", ctx),
            });
        }
    }

    if !had_user_placeholder {
        msgs.push(ChatMessage { role: "user".into(), content: user_text.to_string() });
    }

    msgs
}

pub(super) fn build_retry_messages(
    user_text: &str,
    conversation_context: Option<&str>,
) -> Vec<ChatMessage> {
    let base_retry = base_prompt_for_reply(conversation_context);
    let (mut retry_msgs, had_user_placeholder, had_context_placeholder) =
        apply_placeholders(base_retry, user_text, conversation_context);

    retry_msgs.push(ChatMessage {
        role: "system".into(),
        content:
            "2 bullets max. ≤ 60 Japanese chars each. Plain text. No URLs. Unconfirmed future dates → “未確定”."
                .parse()
                .unwrap(),
    });
    retry_msgs.push(ChatMessage {
        role: "system".into(),
        content: format!("CurrentTime(JST): {}", now_tokyo_rfc3339()),
    });

    if let Some(ctx) = conversation_context {
        if !had_context_placeholder {
            retry_msgs.push(ChatMessage {
                role: "system".into(),
                content: format!("[context]\n{}", ctx),
            });
        }
    }
    if !had_user_placeholder {
        retry_msgs.push(ChatMessage { role: "user".into(), content: user_text.to_string() });
    }

    retry_msgs
}

pub(super) fn build_parrot_retry_messages(
    user_text: &str,
    conversation_context: Option<&str>,
) -> Vec<ChatMessage> {
    let base_retry = base_prompt_for_reply(conversation_context);
    let (mut retry_msgs, _had_user_placeholder, _had_context_placeholder) =
        apply_placeholders(base_retry, user_text, conversation_context);

    retry_msgs.push(ChatMessage {
        role: "system".into(),
        content: "さっきの返答はユーザーの発言をそのまま繰り返してしまっていました。今度は必ず質問に答えてください。質問文を\
そのまま返すのではなく、あなたの答えやリアクションを1〜3文で書いてください。".into(),
    });

    retry_msgs
}

fn base_prompt_for_reply(conversation_context: Option<&str>) -> Vec<ChatMessage> {
    if conversation_context.is_some() {
        PROMPTS.reply_with_context.clone()
    } else {
        PROMPTS.reply_without_context.clone()
    }
}

fn search_mandate_instruction() -> String {
    [
        "When asked about versions/release notes/highlights:",
        "• You MUST use web_search to fetch official sources.",
        "• Output: 2 bullets max, plain text only.",
        "• Each bullet ≤ 70 Japanese chars.",
        "• Include the exact version and a YYYY-MM-DD (JST) date.",
        "• Add one source domain in parentheses, e.g., (blog.rust-lang.org).",
        "• Do NOT speculate about future releases.",
        "• If a future date isn't confirmed by official sources, say “未確定”.",
        "• NO URLs and NO markdown. Do not output '[' ']' '(' within URLs.",
        "• Keep total length ≤ 180 Japanese chars.",
        "• Perform at most one search call.",
    ]
    .join(" ")
}

fn apply_placeholders(
    template: Vec<ChatMessage>,
    user_text: &str,
    conversation_context: Option<&str>,
) -> (Vec<ChatMessage>, bool, bool) {
    let context_str = conversation_context.unwrap_or("");
    let mut had_user_placeholder = false;
    let mut had_context_placeholder = false;

    let messages = template
        .into_iter()
        .map(|mut msg| {
            if msg.content.contains("{{USER_TEXT}}") {
                had_user_placeholder = true;
            }
            if msg.content.contains("{{CONTEXT}}") {
                had_context_placeholder = true;
            }
            msg.content =
                msg.content.replace("{{USER_TEXT}}", user_text).replace("{{CONTEXT}}", context_str);
            msg
        })
        .collect();

    (messages, had_user_placeholder, had_context_placeholder)
}
