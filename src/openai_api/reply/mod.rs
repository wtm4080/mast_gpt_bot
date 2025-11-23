use anyhow::Result;
use reqwest::Client;

use crate::config::BotConfig;
use crate::openai_api::stream::{CallResponsesArgs, call_responses};
use crate::openai_api::types::{ChatMessage, ResponsesResult, Tool};

use self::message_builder::{
    build_initial_messages, build_parrot_retry_messages, build_retry_messages,
};
use self::parrot_check::is_parrot_reply;
use self::search::should_force_search;

mod message_builder;
mod parrot_check;
mod search;
mod time;

pub struct ReplyResult {
    pub text: String,
    pub response_id: String,
}

pub async fn generate_reply(
    client: &Client,
    cfg: &BotConfig,
    user_text: &str,
    conversation_context: Option<&str>,
    previous_response_id: Option<String>,
) -> Result<ReplyResult> {
    let force_search = should_force_search(user_text);

    let model = &cfg.openai_model;
    let model_reply = &cfg.openai_reply_model;
    let api_key = &cfg.openai_api_key;

    let messages: Vec<ChatMessage> =
        build_initial_messages(user_text, conversation_context, force_search);

    let mut tools = Vec::new();
    if cfg.enable_web_search || force_search {
        tools.push(Tool::WebSearchPreview { search_context_size: Some("low".into()) });
    }

    let mut builder = CallResponsesArgs::new(model, model_reply, api_key, messages)
        .temperature(cfg.reply_temperature)
        .max_output_tokens(140);

    if let Some(prev) = previous_response_id {
        builder = builder.previous_response_id(prev);
    }
    if !tools.is_empty() {
        builder = builder.tools(tools);
    }

    let mut res: ResponsesResult = call_responses(client, builder, true).await?;

    if res.text.trim().is_empty() || res.status.as_deref() == Some("incomplete") {
        let retry_msgs = build_retry_messages(user_text, conversation_context);

        let mut retry_tools = Vec::new();
        if cfg.enable_web_search || force_search {
            retry_tools.push(Tool::WebSearchPreview { search_context_size: Some("low".into()) });
        }

        let mut retry_builder = CallResponsesArgs::new(model, model_reply, api_key, retry_msgs)
            .temperature(cfg.reply_temperature)
            .max_output_tokens(120);
        if !retry_tools.is_empty() {
            retry_builder = retry_builder.tools(retry_tools);
        }

        let retry_res: ResponsesResult = call_responses(client, retry_builder, true).await?;
        if !retry_res.text.trim().is_empty() {
            res = retry_res;
        }
    }

    if !force_search && is_parrot_reply(user_text, res.text.trim()) {
        let retry_msgs = build_parrot_retry_messages(user_text, conversation_context);

        let retry_builder = CallResponsesArgs::new(model, model_reply, api_key, retry_msgs)
            .temperature(cfg.reply_temperature)
            .max_output_tokens(1024);

        let retry_res: ResponsesResult = call_responses(client, retry_builder, true).await?;
        if !retry_res.text.trim().is_empty() {
            res = retry_res;
        }
    }

    let clean = res.text.trim();
    let final_text = if clean.starts_with('{') || clean.starts_with('[') {
        "短く要点＋出典ドメインでまとめられなかったみたい。もう一度聞いて！".to_string()
    } else {
        clean.to_string()
    };

    Ok(ReplyResult { text: final_text, response_id: res.id })
}
