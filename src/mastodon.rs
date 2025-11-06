///! Mastodon API まわり（型＋HTTP）

use anyhow::{Context, Result};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct Notification {
    pub id: String,
    #[serde(rename = "type")]
    pub notif_type: String,
    pub status: Option<Status>,
    pub account: Account,
}

#[derive(Debug, Deserialize)]
pub struct Status {
    pub id: String,
    pub content: String, // HTML
    pub visibility: String,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize)]
struct NewStatusPlain<'a> {
    status: &'a str,
    visibility: &'a str,
}

/// メンション通知を取得
pub async fn fetch_mentions(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    since_id: Option<&str>,
) -> Result<Vec<Notification>> {
    let mut url = format!("{}/api/v1/notifications?types[]=mention&limit=20", base_url);
    if let Some(since) = since_id {
        url.push_str("&since_id=");
        url.push_str(since);
    }

    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .context("Mastodon notifications request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Mastodon error {}: {}", status, text);
    }

    let notifs: Vec<Notification> = resp.json().await.context("parse notifications")?;
    Ok(notifs)
}

/// 返信を投稿
pub async fn post_reply(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    reply_to: &Status,
    reply_to_acct: &str,
    body: &str,
) -> Result<()> {
    let url = format!("{}/api/v1/statuses", base_url);

    let status_text = format!("@{} {}", reply_to_acct, body);

    let new_status = NewStatusReply {
        status: &status_text,
        in_reply_to_id: &reply_to.id,
        visibility: &reply_to.visibility,
    };

    let resp = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&new_status)
        .send()
        .await
        .context("Mastodon post status failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Mastodon post error {}: {}", status, text);
    }

    Ok(())
}

/// 自由ポスト（返信じゃない普通のトゥート）を投稿
pub async fn post_status(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    body: &str,
    visibility: &str,
) -> Result<()> {
    let url = format!("{}/api/v1/statuses", base_url);

    let new_status = NewStatusPlain { status: body, visibility };

    let resp = client
        .post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .json(&new_status)
        .send()
        .await
        .context("Mastodon post status (free toot) failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Mastodon post error {}: {}", status, text);
    }

    Ok(())
}
