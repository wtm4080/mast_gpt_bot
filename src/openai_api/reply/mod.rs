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

async fn call_initial_reply(
    client: &Client,
    call_config: &ReplyCallConfig<'_>,
    user_text: &str,
    conversation_context: Option<&str>,
    force_search: bool,
    previous_response_id: Option<String>,
    web_search_tools: &[Tool],
) -> Result<ResponsesResult> {
    let messages = build_initial_messages(user_text, conversation_context, force_search);
    let builder = build_reply_call(
        call_config,
        messages,
        140,
        previous_response_id,
        web_search_tools.to_vec(),
    );

    call_responses(client, builder, true).await
}

async fn retry_empty_or_incomplete_reply(
    client: &Client,
    call_config: &ReplyCallConfig<'_>,
    user_text: &str,
    conversation_context: Option<&str>,
    current: ResponsesResult,
    web_search_tools: Vec<Tool>,
) -> Result<ResponsesResult> {
    if !should_retry_empty_or_incomplete(&current) {
        return Ok(current);
    }

    let retry_msgs = build_retry_messages(user_text, conversation_context);
    let retry_builder = build_reply_call(call_config, retry_msgs, 120, None, web_search_tools);
    let retry_res = call_responses(client, retry_builder, true).await?;

    Ok(prefer_non_empty_retry(current, retry_res))
}

async fn retry_parrot_reply(
    client: &Client,
    call_config: &ReplyCallConfig<'_>,
    user_text: &str,
    conversation_context: Option<&str>,
    force_search: bool,
    current: ResponsesResult,
) -> Result<ResponsesResult> {
    if !should_retry_parrot(force_search, user_text, current.text.trim()) {
        return Ok(current);
    }

    let retry_msgs = build_parrot_retry_messages(user_text, conversation_context);
    let retry_builder = build_reply_call(call_config, retry_msgs, 1024, None, Vec::new());
    let retry_res = call_responses(client, retry_builder, true).await?;

    Ok(prefer_non_empty_retry(current, retry_res))
}

fn prefer_non_empty_retry(current: ResponsesResult, retry: ResponsesResult) -> ResponsesResult {
    if retry.text.trim().is_empty() { current } else { retry }
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

    let web_search_tools = build_web_search_tools(cfg.enable_web_search, force_search);

    let res = call_initial_reply(
        client,
        &call_config,
        user_text,
        conversation_context,
        force_search,
        previous_response_id,
        &web_search_tools,
    )
    .await?;
    let res = retry_empty_or_incomplete_reply(
        client,
        &call_config,
        user_text,
        conversation_context,
        res,
        web_search_tools,
    )
    .await?;
    let res = retry_parrot_reply(
        client,
        &call_config,
        user_text,
        conversation_context,
        force_search,
        res,
    )
    .await?;

    let final_text = final_reply_text(&res.text);

    Ok(ReplyResult { text: final_text, response_id: res.id })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { role: role.to_string(), content: content.to_string() }
    }

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

    #[test]
    fn reply_call_builder_preserves_config_and_optional_fields() {
        let call_config = ReplyCallConfig {
            model: "base-model",
            model_reply: "reply-model",
            api_key: "api-key",
            temperature: 0.5,
        };
        let tools = vec![Tool::WebSearchPreview { search_context_size: Some("low".into()) }];

        let args = build_reply_call(
            &call_config,
            vec![message("user", "hello")],
            140,
            Some("resp_prev".into()),
            tools,
        );

        assert_eq!(args.model, "base-model");
        assert_eq!(args.model_reply, "reply-model");
        assert_eq!(args.api_key, "api-key");
        assert_eq!(args.temperature, Some(0.5));
        assert_eq!(args.max_output_tokens, Some(140));
        assert_eq!(args.previous_response_id.as_deref(), Some("resp_prev"));
        assert_eq!(args.messages.len(), 1);
        assert!(args.tools.is_some());
    }
}
