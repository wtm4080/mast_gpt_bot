///! Mastodon API まわり（型＋HTTP）

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::config::BotConfig;

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
    pub content: String,   // HTML
    pub visibility: String,
    #[allow(dead_code)]
    pub in_reply_to_id: Option<String>,
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
    token: &str,
    status_id: &str,
) -> Result<StatusContext> {
    let url = format!("{}/api/v1/statuses/{}/context", base_url, status_id);

    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .context("Mastodon status context request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Mastodon context error {}: {}", status, text);
    }

    let ctx: StatusContext = resp.json().await.context("parse status context")?;
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
pub async fn post_status(client: &Client, cfg: &BotConfig, text: &str) -> Result<()> {
    // エンドポイント
    let url = format!("{}/api/v1/statuses", cfg.mastodon_base);

    // 可視性（Visibility -> &str 変換）
    // 既存の Visibility が Display 実装済みなら to_string() でOK。
    // 未実装なら match で文字列に落とす。
    let visibility_str = cfg.visibility.to_string();

    // 本文が空は弾く（念のため）
    let status = text.trim();
    if status.is_empty() {
        return Err(anyhow!("post_status: empty status"));
    }

    // リクエスト作成
    let req = client
        .post(&url)
        .bearer_auth(&cfg.mastodon_access_token)
        .form(&json!({
            "status": status,
            "visibility": visibility_str,
        }));

    // 送信
    let resp = req.send().await?;
    if !resp.status().is_success() {
        let code = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("post_status: http {}: {}", code, body));
    }

    Ok(())
}
