use super::BotConfig;

pub struct Redacted<'a>(pub(crate) &'a BotConfig);

impl std::fmt::Debug for Redacted<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let c = self.0;
        f.debug_struct("BotConfig")
            .field("mastodon_base", &c.mastodon_base)
            .field("mastodon_token", &mask(&c.mastodon_access_token))
            .field("openai_model", &c.openai_model)
            .field("openai_api_key", &mask(&c.openai_api_key))
            .field("streaming_base_url", &c.streaming_base_url)
            .field("prompts_path", &c.prompts_path)
            .field("bot_db_path", &c.bot_db_path)
            .field("free_toot_interval_secs", &c.free_toot_interval.as_secs())
            .field("reply_temperature", &c.reply_temperature)
            .field("free_toot_temperature", &c.free_toot_temperature)
            .field("visibility", &c.visibility)
            .field("reply_min_interval_ms", &c.reply_min_interval.as_millis())
            .finish()
    }
}

fn mask(s: &str) -> String {
    if s.len() <= 6 { "***".into() } else { format!("{}***", &s[..3]) }
}
