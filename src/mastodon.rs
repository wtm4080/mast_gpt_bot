//! Mastodon API まわり（型＋HTTP）

use crate::config::BotConfig;
use crate::util::fit_for_mastodon_plain;
use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use reqwest::header::AUTHORIZATION;
use reqwest::{RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct Notification {
    pub id: String,
    #[serde(rename = "type")]
    pub notif_type: String,
    pub status: Option<Status>,
    pub account: Account,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Status {
    pub id: String,
    pub content: String, // HTML
    pub visibility: String,
    #[allow(dead_code)]
    pub in_reply_to_id: Option<String>,
    #[allow(dead_code)]
    pub account: Account,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub acct: String,
    pub bot: Option<bool>,
}

#[derive(Debug, Serialize)]
struct NewStatusReply<'a> {
    status: &'a str,
    in_reply_to_id: &'a str,
    visibility: &'a str,
}

fn statuses_url(base_url: &str) -> String {
    format!("{}/api/v1/statuses", base_url)
}

fn status_context_url(base_url: &str, status_id: &str) -> String {
    format!("{}/api/v1/statuses/{}/context", base_url, status_id)
}

fn authenticated_status_post(client: &Client, url: &str, token: &str) -> RequestBuilder {
    client.post(url).header(AUTHORIZATION, format!("Bearer {}", token))
}

fn mastodon_post_error_message(kind: MastodonPostKind, status: StatusCode, body: &str) -> String {
    match kind {
        MastodonPostKind::Reply => format!("Mastodon post error {}: {}", status, body),
        MastodonPostKind::Status => format!("post_status: http {}: {}", status, body),
    }
}

#[derive(Clone, Copy)]
enum MastodonPostKind {
    Reply,
    Status,
}

async fn ensure_mastodon_post_success(
    resp: reqwest::Response,
    kind: MastodonPostKind,
) -> Result<()> {
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!(mastodon_post_error_message(kind, status, &body)));
    }

    Ok(())
}

fn reply_status_text(reply_to_acct: &str, body: &str) -> String {
    format!("@{} {}", reply_to_acct, body)
}

fn new_status_reply<'a>(status_text: &'a str, reply_to: &'a Status) -> NewStatusReply<'a> {
    NewStatusReply {
        status: status_text,
        in_reply_to_id: &reply_to.id,
        visibility: &reply_to.visibility,
    }
}

fn post_status_form(cfg: &BotConfig, text: &str) -> Result<serde_json::Value> {
    let visibility_str = cfg.visibility.to_string();
    let status = fit_for_mastodon_plain(text.trim(), cfg.mastodon_char_limit);
    if status.is_empty() {
        return Err(anyhow!("post_status: empty after fit"));
    }

    Ok(json!({
        "status": status,
        "visibility": visibility_str,
    }))
}

#[derive(Debug, Deserialize)]
pub struct StatusContext {
    pub ancestors: Vec<Status>,
    #[allow(dead_code)]
    pub descendants: Vec<Status>,
}

/// 会話スレッドの文脈（ancestors / descendants）を取得
pub async fn fetch_status_context(
    client: &Client,
    base_url: &str,
    access_token: &str,
    status_id: &str,
) -> Result<StatusContext> {
    let url = status_context_url(base_url, status_id);
    let resp = client.get(&url).bearer_auth(access_token).send().await?.error_for_status()?;

    let ctx: StatusContext = resp.json().await?;
    Ok(ctx)
}

/// 返信を投稿
pub async fn post_reply(
    client: &Client,
    base_url: &str,
    token: &str,
    reply_to: &Status,
    reply_to_acct: &str,
    body: &str,
) -> Result<()> {
    let url = statuses_url(base_url);
    let status_text = reply_status_text(reply_to_acct, body);
    let new_status = new_status_reply(&status_text, reply_to);

    let resp = authenticated_status_post(client, &url, token)
        .json(&new_status)
        .send()
        .await
        .context("Mastodon post status failed")?;

    ensure_mastodon_post_success(resp, MastodonPostKind::Reply).await
}

/// 自由ポスト（返信じゃない普通のトゥート）を投稿
pub async fn post_status(client: &Client, cfg: &BotConfig, text: &str) -> Result<()> {
    let url = statuses_url(&cfg.mastodon_base);
    let form = post_status_form(cfg, text)?;

    let resp = authenticated_status_post(client, &url, &cfg.mastodon_access_token)
        .form(&form)
        .send()
        .await?;

    ensure_mastodon_post_success(resp, MastodonPostKind::Status).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_config;

    #[test]
    fn statuses_url_appends_statuses_endpoint_to_base_url() {
        assert_eq!(
            statuses_url("https://mastodon.example"),
            "https://mastodon.example/api/v1/statuses"
        );
    }

    #[test]
    fn status_context_url_appends_context_endpoint_to_status() {
        assert_eq!(
            status_context_url("https://mastodon.example", "status-1"),
            "https://mastodon.example/api/v1/statuses/status-1/context"
        );
    }

    #[test]
    fn post_status_form_preserves_visibility_and_fits_status() {
        let cfg = test_config();

        let form = post_status_form(&cfg, " hello ").unwrap();

        assert_eq!(form["status"], "hello");
        assert_eq!(form["visibility"], "unlisted");
    }

    #[test]
    fn new_status_reply_mentions_account_and_uses_reply_visibility() {
        let reply_to = Status {
            id: "status-1".to_string(),
            content: "<p>hello</p>".to_string(),
            visibility: "private".to_string(),
            in_reply_to_id: None,
            account: Account { acct: "alice".to_string(), bot: Some(false) },
        };
        let status_text = reply_status_text("alice", "thanks");
        let new_status = new_status_reply(&status_text, &reply_to);

        assert_eq!(new_status.status, "@alice thanks");
        assert_eq!(new_status.in_reply_to_id, "status-1");
        assert_eq!(new_status.visibility, "private");
    }

    #[test]
    fn mastodon_post_error_message_preserves_existing_formats() {
        let status = StatusCode::BAD_REQUEST;

        assert_eq!(
            mastodon_post_error_message(MastodonPostKind::Reply, status, "bad"),
            "Mastodon post error 400 Bad Request: bad"
        );
        assert_eq!(
            mastodon_post_error_message(MastodonPostKind::Status, status, "bad"),
            "post_status: http 400 Bad Request: bad"
        );
    }

    #[tokio::test]
    async fn post_status_rejects_empty_text_after_fitting_before_http() {
        let client = Client::new();
        let cfg = test_config();

        let err = post_status(&client, &cfg, "   ").await.unwrap_err();

        assert_eq!(err.to_string(), "post_status: empty after fit");
    }
}
