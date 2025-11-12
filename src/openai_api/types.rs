use serde::{Deserialize, Serialize};

/// Responses API の input として投げるメッセージ
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// /v1/responses のリクエストボディ
#[derive(Debug, Serialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

/// output[*].content[*] の中身
#[derive(Debug, Deserialize)]
pub struct ResponsesContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

/// output[*]
#[derive(Debug, Deserialize)]
pub struct ResponsesOutputItem {
    #[allow(dead_code)]
    pub role: Option<String>,
    pub content: Vec<ResponsesContent>,
}

/// /v1/responses のレスポンス全体
#[derive(Debug, Deserialize)]
pub struct ResponsesResponse {
    pub output: Vec<ResponsesOutputItem>,
}
