use anyhow::Result;
use reqwest::Client;

use crate::openai_api::stream::chat_stream;
use crate::openai_api::types::ChatMessage;

pub async fn generate_reply(
    client: &Client,
    model: &str,
    api_key: &str,
    user_text: &str,
    conversation_context: Option<&str>,
) -> Result<String> {
    let system_msg = ChatMessage {
        role: "system".into(),
        content: "あなたは Mastodon のタイムラインでゆるく喋る日本語話者です。丁寧すぎない口調で、相手を安心させる感じで返信してください。ただし失礼な言い方や攻撃的な表現はしないでください。"
            .into(),
    };

    let user_msg = if let Some(ctx) = conversation_context {
        ChatMessage {
            role: "user".into(),
            content: format!(
                concat!(
                "以下は、これまでの会話の流れです（古い順）。一番下が相手の最新の投稿です:\n",
                "{}\n\n",
                "この会話の流れを踏まえて、相手の最新の投稿に対する返信として、",
                "Mastodon に投稿できる短めのメッセージを書いてください。\n",
                "ただし、あなたが直前に言った内容をそのまま繰り返さないでください。\n",
                "相手の最新の投稿が「なるほど」「え？」「草」「www」などの短い相槌だけの場合は、\n",
                "軽い相槌や一言リアクション、あるいは少しだけ話題を膨らませる返信にしてください。\n",
                "説教くさい言い方や、同じフレーズの繰り返しは避けてください。",
                ),
                ctx
            ),
        }
    } else {
        ChatMessage {
            role: "user".into(),
            content: format!(
                concat!(
                "自由につぶやいてください。相手の投稿への返信として一言を書いてください。\n",
                "同じ質問に対しても、できるだけ毎回少し表現を変えてください。\n",
                "必要があれば、もう1〜2文だけ軽く説明を足してもOKです。\n",
                "\n",
                "相手の投稿: {}\n",
                ),
                user_text
            ),
        }
    };

    let messages = vec![system_msg, user_msg];

    // 会話返信はちょい変化欲しいので 0.6 前後
    chat_stream(client, model, api_key, messages, Some(0.6)).await
}
