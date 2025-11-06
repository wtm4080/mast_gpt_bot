///! 共通設定用の構造体

#[derive(Clone)]
pub struct BotConfig {
    pub mastodon_base: String,
    pub mastodon_token: String,
    pub openai_model: String,
    pub openai_api_key: String,
    pub post_visibility: String,
    pub streaming_base_url: String,
}
