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

const JSON_FALLBACK_REPLY: &str =
    "短く要点＋出典ドメインでまとめられなかったみたい。もう一度聞いて！";

fn build_web_search_tools(enable_web_search: bool, force_search: bool) -> Vec<Tool> {
    if enable_web_search || force_search {
        vec![Tool::WebSearchPreview { search_context_size: Some("low".into()) }]
    } else {
        Vec::new()
    }
}

fn should_retry_empty_or_incomplete(res: &ResponsesResult) -> bool {
    res.text.trim().is_empty() || res.status.as_deref() == Some("incomplete")
}

fn should_retry_parrot(force_search: bool, user_text: &str, reply_text: &str) -> bool {
    !force_search && is_parrot_reply(user_text, reply_text)
}

fn final_reply_text(text: &str) -> String {
    let clean = text.trim();
    if clean.starts_with('{') || clean.starts_with('[') {
        JSON_FALLBACK_REPLY.to_string()
    } else {
        clean.to_string()
    }
}

struct ReplyCallConfig<'a> {
    model: &'a str,
    model_reply: &'a str,
    api_key: &'a str,
    temperature: f32,
}

fn build_reply_call<'a>(
    call_config: &ReplyCallConfig<'a>,
    messages: Vec<ChatMessage>,
    max_output_tokens: u32,
    previous_response_id: Option<String>,
    tools: Vec<Tool>,
) -> CallResponsesArgs<'a> {
    let mut builder = CallResponsesArgs::new(
        call_config.model,
        call_config.model_reply,
        call_config.api_key,
        messages,
    )
    .temperature(call_config.temperature)
    .max_output_tokens(max_output_tokens);

    if let Some(prev) = previous_response_id {
        builder = builder.previous_response_id(prev);
    }
    if !tools.is_empty() {
        builder = builder.tools(tools);
    }

    builder
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
    let call_config =
        ReplyCallConfig { model, model_reply, api_key, temperature: cfg.reply_temperature };

    let messages: Vec<ChatMessage> =
        build_initial_messages(user_text, conversation_context, force_search);
    let web_search_tools = build_web_search_tools(cfg.enable_web_search, force_search);

    let builder = build_reply_call(
        &call_config,
        messages,
        140,
        previous_response_id,
        web_search_tools.clone(),
    );

    let mut res: ResponsesResult = call_responses(client, builder, true).await?;

    if should_retry_empty_or_incomplete(&res) {
        let retry_msgs = build_retry_messages(user_text, conversation_context);

        let retry_builder = build_reply_call(&call_config, retry_msgs, 120, None, web_search_tools);

        let retry_res: ResponsesResult = call_responses(client, retry_builder, true).await?;
        if !retry_res.text.trim().is_empty() {
            res = retry_res;
        }
    }

    if should_retry_parrot(force_search, user_text, res.text.trim()) {
        let retry_msgs = build_parrot_retry_messages(user_text, conversation_context);

        let retry_builder = build_reply_call(&call_config, retry_msgs, 1024, None, Vec::new());

        let retry_res: ResponsesResult = call_responses(client, retry_builder, true).await?;
        if !retry_res.text.trim().is_empty() {
            res = retry_res;
        }
    }

    let final_text = final_reply_text(&res.text);

    Ok(ReplyResult { text: final_text, response_id: res.id })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(text: &str, status: Option<&str>) -> ResponsesResult {
        ResponsesResult {
            id: "resp_1".to_string(),
            text: text.to_string(),
            status: status.map(|s| s.to_string()),
        }
    }

    #[test]
    fn retries_empty_or_incomplete_responses() {
        assert!(should_retry_empty_or_incomplete(&response("  ", Some("completed"))));
        assert!(should_retry_empty_or_incomplete(&response("text", Some("incomplete"))));
        assert!(!should_retry_empty_or_incomplete(&response("text", Some("completed"))));
        assert!(!should_retry_empty_or_incomplete(&response("text", None)));
    }

    #[test]
    fn parrot_retry_is_disabled_when_search_is_forced() {
        assert!(should_retry_parrot(false, "hello", "hello"));
        assert!(!should_retry_parrot(true, "hello", "hello"));
    }

    #[test]
    fn final_reply_trims_text_and_replaces_json_like_output() {
        assert_eq!(final_reply_text("  hello  "), "hello");
        assert_eq!(final_reply_text("{\"text\":\"hello\"}"), JSON_FALLBACK_REPLY);
        assert_eq!(final_reply_text("[hello]"), JSON_FALLBACK_REPLY);
    }

    #[test]
    fn web_search_tools_are_enabled_by_config_or_forced_search() {
        assert!(build_web_search_tools(false, false).is_empty());

        for tools in [build_web_search_tools(true, false), build_web_search_tools(false, true)] {
            assert_eq!(tools.len(), 1);
            match &tools[0] {
                Tool::WebSearchPreview { search_context_size } => {
                    assert_eq!(search_context_size.as_deref(), Some("low"));
                }
            }
        }
    }
}
