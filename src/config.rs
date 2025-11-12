use anyhow::{Context, Result, anyhow, bail};
use std::fmt::Display;
use std::{env, str::FromStr, time::Duration};

#[derive(Clone, Debug)]
pub struct BotConfig {
    // --- 必須 ---
    pub mastodon_base: String, // 例: https://mastodon.social
    pub mastodon_access_token: String,
    pub openai_model: String,
    pub openai_api_key: String,

    // --- 任意（デフォルトあり）---
    pub streaming_base_url: String, // 例: wss://mastodon.social
    pub prompts_path: String,       // 例: config/prompts.json
    pub bot_db_path: String,        // 例: bot_state.sqlite

    pub free_toot_interval: Duration,
    pub reply_temperature: f32,
    pub free_toot_temperature: f32,

    pub visibility: Visibility, // 投稿公開範囲

    pub reply_min_interval: Duration,

    // Tools
    pub enable_web_search: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Visibility {
    Public,
    Unlisted,
    Private,
    Direct,
}

impl FromStr for Visibility {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "public" => Ok(Self::Public),
            "unlisted" => Ok(Self::Unlisted),
            "private" => Ok(Self::Private),
            "direct" => Ok(Self::Direct),
            other => bail!("unknown VISIBILITY: {other}"),
        }
    }
}

impl Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Visibility::Public => "public",
            Visibility::Unlisted => "unlisted",
            Visibility::Private => "private",
            Visibility::Direct => "direct",
        };

        write!(f, "{}", s.to_string())
    }
}

impl BotConfig {
    pub fn from_env() -> Result<Self> {
        // .env 読み込み（なければ無視）
        let _ = dotenvy::from_filename(".env");

        // 必須
        let mastodon_base = must("MASTODON_BASE_URL")?;
        let mastodon_token = must("MASTODON_ACCESS_TOKEN")?;
        let openai_model = must("OPENAI_MODEL")?;
        let openai_api_key = must("OPENAI_API_KEY")?;

        // 任意
        let streaming_base_url =
            opt("#MASTODON_STREAMING_URL").unwrap_or_else(|| default_streaming_ws(&mastodon_base));
        let prompts_path = opt("PROMPTS_PATH").unwrap_or_else(|| "config/prompts.json".into());
        let bot_db_path = opt("BOT_DB_PATH").unwrap_or_else(|| "bot_state.sqlite".into());

        let free_toot_interval: u64 = parse("FREE_TOOT_INTERVAL_SECS", 3600)?;
        let free_toot_interval = Duration::from_secs(free_toot_interval);

        let reply_temperature: f32 = parse("REPLY_TEMPERATURE", 0.7)?;
        let free_toot_temperature: f32 = parse("FREE_TOOT_TEMPERATURE", 0.8)?;

        let visibility: Visibility = parse_str("MASTODON_POST_VISIBILITY", "unlisted")?;

        let reply_min_interval_ms: u64 = parse("REPLY_MIN_INTERVAL_MS", 3000)?;
        let reply_min_interval = Duration::from_millis(reply_min_interval_ms);

        // tools
        let enable_web_search: bool = parse("ENABLE_WEB_SEARCH", false)?;

        Ok(Self {
            mastodon_base,
            mastodon_access_token: mastodon_token,
            openai_model,
            openai_api_key,
            streaming_base_url,
            prompts_path,
            bot_db_path,
            free_toot_interval,
            reply_temperature,
            free_toot_temperature,
            visibility,
            reply_min_interval,
            enable_web_search,
        })
    }

    /// ログに出す用（秘密を伏せる）
    pub fn redacted(&self) -> Redacted<'_> {
        Redacted(self)
    }
}

// ── 補助 ───────────────────────────────────────────

fn must(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("missing required env: {key}"))
}

fn opt(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty())
}

fn parse<T: FromStr>(key: &str, default: T) -> Result<T>
where
    <T as FromStr>::Err: Display, // エラーメッセージ整形用
{
    match opt(key) {
        Some(s) => s.parse::<T>().map_err(|e| anyhow!("failed to parse {key}='{s}': {e}")),
        None => Ok(default),
    }
}

fn parse_str<T: FromStr>(key: &str, default: &str) -> Result<T>
where
    <T as FromStr>::Err: Display,
{
    match opt(key) {
        Some(s) => s.parse::<T>().map_err(|e| anyhow!("failed to parse {key}='{s}': {e}")),
        None => default
            .parse::<T>()
            .map_err(|e| anyhow!("failed to parse default of {key} ('{default}'): {e}")),
    }
}

fn default_streaming_ws(http_base: &str) -> String {
    // http(s) → ws(s) に置換して使いやすい既定値を作る
    // wss://kirishima.cloud/api/v1/streaming
    if let Some(rest) = http_base.strip_prefix("https://") {
        format!("wss://{rest}/api/v1/streaming")
    } else if let Some(rest) = http_base.strip_prefix("http://") {
        format!("ws://{rest}/api/v1/streaming")
    } else {
        // もともと wss:// を直で渡してるケースにも対応
        http_base.to_string()
    }
}

/// 機微情報をマスクしてデバッグ表示
pub struct Redacted<'a>(&'a BotConfig);
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
