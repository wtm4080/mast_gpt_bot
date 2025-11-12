use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ChatStreamChoiceDelta {
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatStreamChoice {
    pub delta: ChatStreamChoiceDelta,
}

#[derive(Debug, Deserialize)]
pub struct ChatStreamResponse {
    pub choices: Vec<ChatStreamChoice>,
}
