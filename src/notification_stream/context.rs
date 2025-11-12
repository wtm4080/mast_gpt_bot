use crate::mastodon::{Status, StatusContext};
use crate::util::strip_html;

pub fn format_conversation_context(ctx: &StatusContext, current: &Status) -> String {
    let ancestors = &ctx.ancestors;

    // 遡って見る最大件数（あなたが今 10 にしてるやつ）
    let max_back = 10;
    let start = if ancestors.len() > max_back {
        ancestors.len() - max_back
    } else {
        0
    };

    let mut lines = Vec::new();

    for s in &ancestors[start..] {
        let text = strip_html(&s.content);
        if !text.is_empty() {
            lines.push(text);
        }
    }

    let current_text = strip_html(&current.content);
    if !current_text.is_empty() {
        lines.push(current_text);
    }

    lines
        .into_iter()
        .map(|t| format!("- {}", t))
        .collect::<Vec<_>>()
        .join("\n")
}
