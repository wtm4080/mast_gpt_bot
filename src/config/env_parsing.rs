use anyhow::{Context, Result, anyhow};
use std::{env, fmt::Display, str::FromStr};

pub fn must(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("missing required env: {key}"))
}

pub fn opt(key: &str) -> Option<String> {
    env::var(key).ok().filter(|v| !v.is_empty())
}

pub fn parse<T: FromStr>(key: &str, default: T) -> Result<T>
where
    <T as FromStr>::Err: Display,
{
    match opt(key) {
        Some(s) => s.parse::<T>().map_err(|e| anyhow!("failed to parse {key}='{s}': {e}")),
        None => Ok(default),
    }
}

pub fn parse_str<T: FromStr>(key: &str, default: &str) -> Result<T>
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

pub fn default_streaming_ws(http_base: &str) -> String {
    if let Some(rest) = http_base.strip_prefix("https://") {
        format!("wss://{rest}/api/v1/streaming")
    } else if let Some(rest) = http_base.strip_prefix("http://") {
        format!("ws://{rest}/api/v1/streaming")
    } else {
        http_base.to_string()
    }
}
