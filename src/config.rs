///! 共通設定用の構造体

#[derive(Clone)]
pub struct BotConfig {
    pub mastodon_base: String,
    pub mastodon_token: String,
    pub openai_model: String,
    pub openai_api_key: String,
    pub post_visibility: String,
    pub streaming_base_url: String,

    /// リプライ用の最小インターバル（ミリ秒）
    pub reply_min_interval_ms: u64,

    pub reply_temperature: f32,
    pub free_toot_temperature: f32,
}
