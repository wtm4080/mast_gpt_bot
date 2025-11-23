use crate::config::{Redacted, Visibility, env_parsing};
use anyhow::Result;
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct BotConfig {
    // --- 必須 ---
    pub mastodon_base: String, // 例: https://mastodon.social
    pub mastodon_access_token: String,

    /// 自由トゥート用（FTモデル想定）
    pub openai_model: String,
    /// リプライ用（ベースモデル）
    #[serde(default = "default_reply_model")]
    pub openai_reply_model: String,

    pub openai_api_key: String,

    // --- 任意（デフォルトあり）---
    pub streaming_base_url: String, // 例: wss://mastodon.social
    pub prompts_path: String,       // 例: config/prompts.json
    pub bot_db_path: String,        // 例: bot_state.sqlite

    pub free_toot_interval: Duration,
    pub reply_temperature: f32,
    pub free_toot_temperature: f32,

    pub visibility: Visibility, // 投稿公開範囲
    pub mastodon_char_limit: usize,

    pub reply_min_interval: Duration,

    // Tools
    pub enable_web_search: bool,
}

fn default_reply_model() -> String {
    "gpt-4.1-mini".to_string()
}

impl BotConfig {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::from_filename(".env");

        let mastodon_base = env_parsing::must("MASTODON_BASE_URL")?;
        let mastodon_token = env_parsing::must("MASTODON_ACCESS_TOKEN")?;
        let openai_model = env_parsing::must("OPENAI_MODEL")?;
        let openai_reply_model =
            env_parsing::opt("OPENAI_REPLY_MODEL").unwrap_or_else(|| default_reply_model());
        let openai_api_key = env_parsing::must("OPENAI_API_KEY")?;

        let streaming_base_url = env_parsing::opt("MASTODON_STREAMING_URL")
            .unwrap_or_else(|| env_parsing::default_streaming_ws(&mastodon_base));
        let prompts_path =
            env_parsing::opt("PROMPTS_PATH").unwrap_or_else(|| "config/prompts.json".into());
        let bot_db_path =
            env_parsing::opt("BOT_DB_PATH").unwrap_or_else(|| "bot_state.sqlite".into());

        let free_toot_interval: u64 = env_parsing::parse("FREE_TOOT_INTERVAL_SECS", 3600)?;
        let free_toot_interval = Duration::from_secs(free_toot_interval);

        let reply_temperature: f32 = env_parsing::parse("REPLY_TEMPERATURE", 0.7)?;
        let free_toot_temperature: f32 = env_parsing::parse("FREE_TOOT_TEMPERATURE", 0.8)?;

        let visibility: Visibility =
            env_parsing::parse_str("MASTODON_POST_VISIBILITY", "unlisted")?;
        let mastodon_char_limit: usize = env_parsing::parse("MASTODON_CHAR_LIMIT", 500)?;

        let reply_min_interval_ms: u64 = env_parsing::parse("REPLY_MIN_INTERVAL_MS", 3000)?;
        let reply_min_interval = Duration::from_millis(reply_min_interval_ms);

        let enable_web_search: bool = env_parsing::parse("ENABLE_WEB_SEARCH", false)?;

        Ok(Self {
            mastodon_base,
            mastodon_access_token: mastodon_token,
            openai_model,
            openai_reply_model,
            openai_api_key,
            streaming_base_url,
            prompts_path,
            bot_db_path,
            free_toot_interval,
            reply_temperature,
            free_toot_temperature,
            visibility,
            mastodon_char_limit,
            reply_min_interval,
            enable_web_search,
        })
    }

    pub fn redacted(&self) -> Redacted<'_> {
        Redacted(self)
    }
}
