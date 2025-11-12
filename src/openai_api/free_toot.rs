use anyhow::Result;
use reqwest::Client;

use crate::openai_api::stream::chat_stream;
use crate::openai_api::types::ChatMessage;

pub async fn generate_free_toot(
    client: &Client,
    model: &str,
    api_key: &str,
) -> Result<String> {
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "あなたは Mastodon に投稿する日本語話者です。普段タイムラインに流しているような、短めのつぶやきをしてください。口調はややくだけていて、でも攻撃的ではなく、ゆるい独り言っぽさを大事にしてください。"
                .into(),
        },
        ChatMessage {
            role: "user".into(),
            content: "今の気分で、なにか一言つぶやいてみて。".into(),
        },
    ];

    // 自由トゥートはちょっと遊ばせて 0.7 くらい
    chat_stream(client, model, api_key, messages, Some(0.7)).await
}
