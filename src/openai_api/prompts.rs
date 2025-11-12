use crate::openai_api::types::ChatMessage;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Deserialize)]
pub struct PromptConfig {
    pub free_toot_morning: Vec<ChatMessage>,
    pub free_toot_day: Vec<ChatMessage>,
    pub free_toot_night: Vec<ChatMessage>,

    pub reply_with_context: Vec<ChatMessage>,
    pub reply_without_context: Vec<ChatMessage>,
}

/// 相対パスが指定された場合に、
/// 1. カレントディレクトリ基準
/// 2. 実行ファイルのあるディレクトリ基準
/// の順に試す
fn resolve_prompts_path(raw: &str) -> PathBuf {
    let p = Path::new(raw);

    // 絶対パスならそのまま
    if p.is_absolute() {
        return p.to_path_buf();
    }

    // 1. 現在のカレントディレクトリからの相対パス
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let from_cwd = cwd.join(p);
    if from_cwd.exists() {
        return from_cwd;
    }

    // 2. 実行ファイルと同じディレクトリからの相対パス
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let from_exe = exe_dir.join(p);
            if from_exe.exists() {
                return from_exe;
            }
        }
    }

    // どちらにも存在しない場合は、とりあえず cwd 基準を返す
    // （後で read_to_string がエラーを出す）
    cwd.join(p)
}

/// 起動後に最初にアクセスされたタイミングで prompts.json を読み込む。
/// パスは環境変数 PROMPTS_PATH で上書き可能。デフォルトは ./prompts.json
pub static PROMPTS: Lazy<PromptConfig> = Lazy::new(|| {
    let raw_path = std::env::var("PROMPTS_PATH").unwrap_or_else(|_| "prompts.json".to_string());
    let resolved = resolve_prompts_path(&raw_path);

    let data = fs::read_to_string(&resolved).unwrap_or_else(|e| {
        panic!(
            "Failed to read prompts JSON.\n  tried: {}\n  (from PROMPTS_PATH = {:?})\n  error: {}",
            resolved.display(),
            raw_path,
            e
        )
    });

    serde_json::from_str::<PromptConfig>(&data).unwrap_or_else(|e| {
        panic!(
            "Failed to parse prompts JSON {}\n  error: {}",
            resolved.display(),
            e
        )
    })
});
