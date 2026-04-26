use crate::openai_api::prompts::PROMPTS;
use crate::openai_api::types::ChatMessage;

use super::time::now_tokyo_rfc3339;

pub(super) fn build_initial_messages(
    user_text: &str,
    conversation_context: Option<&str>,
    force_search: bool,
) -> Vec<ChatMessage> {
    let (mut msgs, placeholders) = messages_from_reply_template(user_text, conversation_context);

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

    append_missing_context_and_user(&mut msgs, user_text, conversation_context, placeholders);

    msgs
}

pub(super) fn build_retry_messages(
    user_text: &str,
    conversation_context: Option<&str>,
) -> Vec<ChatMessage> {
    let (mut retry_msgs, placeholders) =
        messages_from_reply_template(user_text, conversation_context);

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

    append_missing_context_and_user(&mut retry_msgs, user_text, conversation_context, placeholders);

    retry_msgs
}

pub(super) fn build_parrot_retry_messages(
    user_text: &str,
    conversation_context: Option<&str>,
) -> Vec<ChatMessage> {
    let base_retry = base_prompt_for_reply(conversation_context);
    let (mut retry_msgs, _placeholders) =
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

#[derive(Clone, Copy)]
struct PlaceholderState {
    had_user: bool,
    had_context: bool,
}

fn messages_from_reply_template(
    user_text: &str,
    conversation_context: Option<&str>,
) -> (Vec<ChatMessage>, PlaceholderState) {
    let base = base_prompt_for_reply(conversation_context);
    apply_placeholders(base, user_text, conversation_context)
}

fn append_missing_context_and_user(
    messages: &mut Vec<ChatMessage>,
    user_text: &str,
    conversation_context: Option<&str>,
    placeholders: PlaceholderState,
) {
    if let Some(ctx) = conversation_context
        && !placeholders.had_context
    {
        messages
            .push(ChatMessage { role: "system".into(), content: format!("[context]\n{}", ctx) });
    }

    if !placeholders.had_user {
        messages.push(ChatMessage { role: "user".into(), content: user_text.to_string() });
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
) -> (Vec<ChatMessage>, PlaceholderState) {
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

    (
        messages,
        PlaceholderState { had_user: had_user_placeholder, had_context: had_context_placeholder },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { role: role.to_string(), content: content.to_string() }
    }

    #[test]
    fn apply_placeholders_replaces_user_and_context_and_reports_flags() {
        let template =
            vec![message("system", "ctx={{CONTEXT}}"), message("user", "text={{USER_TEXT}}")];

        let (messages, placeholders) = apply_placeholders(template, "hello", Some("thread"));

        assert!(placeholders.had_user);
        assert!(placeholders.had_context);
        assert_eq!(messages[0].content, "ctx=thread");
        assert_eq!(messages[1].content, "text=hello");
    }

    #[test]
    fn apply_placeholders_replaces_missing_context_with_empty_text() {
        let template = vec![message("system", "ctx={{CONTEXT}}")];

        let (messages, placeholders) = apply_placeholders(template, "hello", None);

        assert!(!placeholders.had_user);
        assert!(placeholders.had_context);
        assert_eq!(messages[0].content, "ctx=");
    }

    #[test]
    fn append_missing_context_and_user_adds_only_missing_inputs() {
        let mut messages = vec![message("system", "base")];

        append_missing_context_and_user(
            &mut messages,
            "hello",
            Some("thread"),
            PlaceholderState { had_user: false, had_context: false },
        );

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].role, "system");
        assert_eq!(messages[1].content, "[context]\nthread");
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content, "hello");

        append_missing_context_and_user(
            &mut messages,
            "ignored",
            Some("ignored"),
            PlaceholderState { had_user: true, had_context: true },
        );

        assert_eq!(messages.len(), 3);
    }
}
