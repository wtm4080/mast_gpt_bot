///! エントリポイント。設定読み込み＆メインループだけ

// src/main.rs
mod mastodon;
mod openai_api;
mod state;
mod util;

use anyhow::Result;
use dotenvy::dotenv;
use mastodon::{fetch_mentions, post_reply, post_status, Notification};
use openai_api::{generate_free_toot, generate_reply};
use state::{load_state, save_state, BotState};
use std::env;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use util::strip_html;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let mastodon_base =
        env::var("MASTODON_BASE_URL").expect("MASTODON_BASE_URL is not set");
    let mastodon_token =
        env::var("MASTODON_ACCESS_TOKEN").expect("MASTODON_ACCESS_TOKEN is not set");
    let openai_api_key =
        env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set");
    let openai_model =
        env::var("OPENAI_MODEL").expect("OPENAI_MODEL is not set (fine-tuned model name)");

    // 自由トゥートの公開範囲
    let post_visibility =
        env::var("MASTODON_POST_VISIBILITY").unwrap_or_else(|_| "public".to_string());

    let client = reqwest::Client::builder()
        .user_agent("mastodon-gpt-bot/0.1")
        .build()?;

    println!("Starting Mastodon GPT bot…");

    let mut state: BotState = load_state();
    println!(
        "Last notification id from state: {:?}",
        state.last_notification_id
    );

    // 起動直後から1回はすぐに自発トゥートさせたくない場合は、
    // ここを Instant::now() に変えると「1時間後の初回ポスト」になる。
    let mut last_free_post = Instant::now() - Duration::from_secs(3600);

    loop {
        if let Err(e) = process_mentions_loop(
            &client,
            &mastodon_base,
            &mastodon_token,
            &openai_model,
            &openai_api_key,
            &mut state,
        )
            .await
        {
            eprintln!("Error in process_mentions_loop: {:?}", e);
        }

        if let Err(e) = hourly_free_toot_loop(
            &client,
            &mastodon_base,
            &mastodon_token,
            &openai_model,
            &openai_api_key,
            &post_visibility,
            &mut last_free_post,
        )
            .await
        {
            eprintln!("Error in hourly_free_toot_loop: {:?}", e);
        }

        // レート制限＆CPU保護
        sleep(Duration::from_secs(15)).await;
    }
}

/// メンションを取得して返信する処理
async fn process_mentions_loop(
    client: &reqwest::Client,
    mastodon_base: &str,
    mastodon_token: &str,
    openai_model: &str,
    openai_api_key: &str,
    state: &mut BotState,
) -> Result<()> {
    let since_id_ref = state.last_notification_id.as_deref();

    let mut notifs =
        fetch_mentions(client, mastodon_base, mastodon_token, since_id_ref).await?;

    // 古い順に処理
    notifs.sort_by_key(|n: &Notification| n.id.clone());

    for notif in notifs {
        // mention 以外はスキップ
        if notif.notif_type != "mention" {
            state.last_notification_id = Some(notif.id.clone());
            save_state(state);
            continue;
        }

        // bot アカウントからのメンションは無視
        if notif.account.bot.unwrap_or(false) {
            println!(
                "Skip mention from bot account @{} (id={})",
                notif.account.acct, notif.id
            );
            state.last_notification_id = Some(notif.id.clone());
            save_state(state);
            continue;
        }

        let status = match &notif.status {
            Some(s) => s,
            None => {
                state.last_notification_id = Some(notif.id.clone());
                save_state(state);
                continue;
            }
        };

        let plain = strip_html(&status.content);
        println!("Mention from @{}: {}", notif.account.acct, plain);

        match generate_reply(client, openai_model, openai_api_key, &plain).await {
            Ok(reply_text) => {
                println!(" -> Reply: {}", reply_text);
                if let Err(e) = post_reply(
                    client,
                    mastodon_base,
                    mastodon_token,
                    status,
                    &notif.account.acct,
                    &reply_text,
                )
                    .await
                {
                    eprintln!("Failed to post reply: {:?}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to generate reply: {:?}", e);
            }
        }

        state.last_notification_id = Some(notif.id.clone());
        save_state(state);
    }

    Ok(())
}

/// 1時間に一度、自由トゥートを投稿する処理
async fn hourly_free_toot_loop(
    client: &reqwest::Client,
    mastodon_base: &str,
    mastodon_token: &str,
    openai_model: &str,
    openai_api_key: &str,
    post_visibility: &str,
    last_free_post: &mut Instant,
) -> Result<()> {
    // まだ1時間経ってなかったら何もしない
    if last_free_post.elapsed() < Duration::from_secs(3600) {
        return Ok(());
    }

    println!("Time for an hourly free toot…");

    match generate_free_toot(client, openai_model, openai_api_key).await {
        Ok(text) => {
            println!("Free toot: {}", text);
            if let Err(e) =
                post_status(client, mastodon_base, mastodon_token, &text, post_visibility).await
            {
                eprintln!("Failed to post free toot: {:?}", e);
            } else {
                *last_free_post = Instant::now();
            }
        }
        Err(e) => {
            eprintln!("Failed to generate free toot: {:?}", e);
            // 失敗したときは last_free_post を更新しないので、次ループで再トライ
        }
    }

    Ok(())
}
