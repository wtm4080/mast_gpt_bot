use crate::openai_api::types::ChatMessage;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
pub struct PromptConfig {
    pub free_toot_morning: Vec<ChatMessage>,
    pub free_toot_day: Vec<ChatMessage>,
    pub free_toot_night: Vec<ChatMessage>,

    pub reply_with_context: Vec<ChatMessage>,
    pub reply_without_context: Vec<ChatMessage>,
}

#[derive(Debug)]
enum PromptLoadError {
    Read { raw_path: String, resolved: PathBuf, source: io::Error },
    Parse { resolved: PathBuf, source: serde_json::Error },
}

impl fmt::Display for PromptLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptLoadError::Read { raw_path, resolved, source } => write!(
                f,
                "Failed to read prompts JSON.\n  tried: {}\n  (from PROMPTS_PATH = {:?})\n  error: {}",
                resolved.display(),
                raw_path,
                source
            ),
            PromptLoadError::Parse { resolved, source } => {
                write!(
                    f,
                    "Failed to parse prompts JSON {}\n  error: {}",
                    resolved.display(),
                    source
                )
            }
        }
    }
}

impl Error for PromptLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PromptLoadError::Read { source, .. } => Some(source),
            PromptLoadError::Parse { source, .. } => Some(source),
        }
    }
}

/// 相対パスが指定された場合に、
///
/// 1. カレントディレクトリ基準
/// 2. 実行ファイルのあるディレクトリ基準
///
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
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let from_exe = exe_dir.join(p);

        if from_exe.exists() {
            return from_exe;
        }
    }

    // どちらにも存在しない場合は、とりあえず cwd 基準を返す
    // （後で read_to_string がエラーを出す）
    cwd.join(p)
}

fn load_prompts_from_path(raw_path: &str) -> std::result::Result<PromptConfig, PromptLoadError> {
    let resolved = resolve_prompts_path(raw_path);

    let data = fs::read_to_string(&resolved).map_err(|source| PromptLoadError::Read {
        raw_path: raw_path.to_string(),
        resolved: resolved.clone(),
        source,
    })?;

    serde_json::from_str::<PromptConfig>(&data)
        .map_err(|source| PromptLoadError::Parse { resolved, source })
}

fn prompts_path_from_env() -> String {
    std::env::var("PROMPTS_PATH").unwrap_or_else(|_| "prompts.json".to_string())
}

/// 起動後に最初にアクセスされたタイミングで prompts.json を読み込む。
/// パスは環境変数 PROMPTS_PATH で上書き可能。デフォルトは ./prompts.json
pub static PROMPTS: Lazy<PromptConfig> = Lazy::new(|| {
    let raw_path = prompts_path_from_env();
    load_prompts_from_path(&raw_path).unwrap_or_else(|e| panic!("{e}"))
});

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("mast_gpt_bot_{name}_{nanos}.json"))
    }

    fn minimal_prompts_json() -> &'static str {
        r#"{
            "free_toot_morning": [{"role": "system", "content": "morning"}],
            "free_toot_day": [{"role": "system", "content": "day"}],
            "free_toot_night": [{"role": "system", "content": "night"}],
            "reply_with_context": [{"role": "system", "content": "with context"}],
            "reply_without_context": [{"role": "system", "content": "without context"}]
        }"#
    }

    #[test]
    fn load_prompts_from_path_reads_and_parses_prompt_config() {
        let path = unique_temp_path("valid_prompts");
        fs::write(&path, minimal_prompts_json()).unwrap();

        let prompts = load_prompts_from_path(path.to_str().unwrap()).unwrap();

        assert_eq!(prompts.free_toot_morning[0].content, "morning");
        assert_eq!(prompts.free_toot_day[0].content, "day");
        assert_eq!(prompts.free_toot_night[0].content, "night");
        assert_eq!(prompts.reply_with_context[0].content, "with context");
        assert_eq!(prompts.reply_without_context[0].content, "without context");

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn load_prompts_from_path_reports_parse_errors_with_path_context() {
        let path = unique_temp_path("invalid_prompts");
        fs::write(&path, "{").unwrap();

        let err = load_prompts_from_path(path.to_str().unwrap()).unwrap_err();
        let message = format!("{err:#}");

        assert!(message.contains("Failed to parse prompts JSON"));
        assert!(message.contains(path.to_str().unwrap()));

        fs::remove_file(path).unwrap();
    }
}
