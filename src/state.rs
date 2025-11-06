///! Bot の永続ステート管理（last_notification_id）

use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BotState {
    pub last_notification_id: Option<String>,
}

const STATE_FILE_PATH: &str = "bot_state.json";

pub fn load_state() -> BotState {
    let path = Path::new(STATE_FILE_PATH);
    if !path.exists() {
        return BotState::default();
    }

    let data = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return BotState::default(),
    };

    match serde_json::from_str::<BotState>(&data) {
        Ok(state) => state,
        Err(_) => BotState::default(),
    }
}

pub fn save_state(state: &BotState) {
    let data = match serde_json::to_string_pretty(state) {
        Ok(s) => s,
        Err(_) => return,
    };
    let _ = fs::write(STATE_FILE_PATH, data);
}
