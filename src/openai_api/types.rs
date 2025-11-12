use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesOutputItem {
    #[allow(unused)]
    pub role: Option<String>,
    pub content: Vec<ResponsesContent>,
}

#[derive(Debug, Deserialize)]
pub struct ResponsesResponse {
    pub id: String,
    pub output: Vec<ResponsesOutputItem>,
}

/// call_responses が返す便利ラッパ
#[derive(Debug)]
pub struct ResponsesResult {
    pub id: String,
    pub text: String,
}
