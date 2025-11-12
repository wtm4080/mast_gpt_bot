use crate::openai_api::types::ChatMessage;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
pub struct PromptConfig {
    pub free_toot: Vec<ChatMessage>,
    pub reply_with_context: Vec<ChatMessage>,
    pub reply_without_context: Vec<ChatMessage>,
}

/// 起動後に最初にアクセスされたタイミングで prompts.json を読み込む。
/// パスは環境変数 PROMPTS_PATH で上書き可能。デフォルトは ./prompts.json
pub static PROMPTS: Lazy<PromptConfig> = Lazy::new(|| {
    let path = std::env::var("PROMPTS_PATH").unwrap_or_else(|_| "config/prompts.json".to_string());
    let path_ref = Path::new(&path);

    let data = fs::read_to_string(path_ref)
        .unwrap_or_else(|e| panic!("Failed to read prompts JSON from {}: {}", path_ref.display(), e));

    serde_json::from_str::<PromptConfig>(&data)
        .unwrap_or_else(|e| panic!("Failed to parse prompts JSON {}: {}", path_ref.display(), e))
});
