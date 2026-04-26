use crate::config::{BotConfig, Visibility};
use std::time::Duration;

pub(crate) fn test_config() -> BotConfig {
    BotConfig {
        mastodon_base: "https://mastodon.example".to_string(),
        mastodon_access_token: "mastodon-token".to_string(),
        openai_model: "gpt-test".to_string(),
        openai_reply_model: "gpt-test-reply".to_string(),
        openai_api_key: "openai-token".to_string(),
        streaming_base_url: "wss://mastodon.example/api/v1/streaming".to_string(),
        prompts_path: "config/prompts.json".to_string(),
        bot_db_path: ":memory:".to_string(),
        free_toot_interval: Duration::from_secs(3600),
        reply_temperature: 0.7,
        free_toot_temperature: 0.8,
        visibility: Visibility::Unlisted,
        mastodon_char_limit: 500,
        reply_min_interval: Duration::from_millis(0),
        enable_web_search: false,
    }
}
