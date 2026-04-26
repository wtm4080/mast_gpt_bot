use crate::config::BotConfig;
use crate::openai_api::stream::CallResponsesArgs;
use crate::openai_api::types::{ChatMessage, Tool};

pub(super) struct OpenAiCallConfig<'a> {
    model: &'a str,
    model_reply: &'a str,
    api_key: &'a str,
    temperature: f32,
}

impl<'a> OpenAiCallConfig<'a> {
    pub(super) fn for_reply(cfg: &'a BotConfig) -> Self {
        Self {
            model: &cfg.openai_model,
            model_reply: &cfg.openai_reply_model,
            api_key: &cfg.openai_api_key,
            temperature: cfg.reply_temperature,
        }
    }

    pub(super) fn for_free_toot(cfg: &'a BotConfig) -> Self {
        Self {
            model: &cfg.openai_model,
            model_reply: &cfg.openai_reply_model,
            api_key: &cfg.openai_api_key,
            temperature: cfg.free_toot_temperature,
        }
    }

    pub(super) fn build(
        &self,
        messages: Vec<ChatMessage>,
        max_output_tokens: u32,
        previous_response_id: Option<String>,
        tools: Vec<Tool>,
    ) -> CallResponsesArgs<'a> {
        let mut builder =
            CallResponsesArgs::new(self.model, self.model_reply, self.api_key, messages)
                .temperature(self.temperature)
                .max_output_tokens(max_output_tokens);

        if let Some(prev) = previous_response_id {
            builder = builder.previous_response_id(prev);
        }
        if !tools.is_empty() {
            builder = builder.tools(tools);
        }

        builder
    }
}

pub(super) fn build_web_search_tools(
    enable_web_search: bool,
    search_context_size: Option<&str>,
) -> Vec<Tool> {
    if enable_web_search {
        vec![Tool::WebSearchPreview {
            search_context_size: search_context_size.map(str::to_string),
        }]
    } else {
        Vec::new()
    }
}
