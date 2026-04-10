use serde::{Deserialize, Serialize};

/// Claude Messages API Request (sent to backend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ClaudeTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ClaudeToolChoice>,
}

/// Claude Message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: ClaudeContent,
}

/// Claude Content - can be string or array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaudeContent {
    Text(String),
    Blocks(Vec<ClaudeContentBlock>),
}

/// Claude Content Block
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaudeContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ClaudeImageSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
}

/// Claude Image Source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

/// Claude Tool Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

/// Claude Tool Choice
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaudeToolChoice {
    Type(String),
    Tool(ClaudeToolChoiceTool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeToolChoiceTool {
    #[serde(rename = "type")]
    pub choice_type: String,
    pub name: String,
}

/// Claude Messages API Response (from backend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ClaudeContentBlock>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ClaudeUsage>,
}

/// Claude Usage Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Claude Streaming Events (from backend)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart {
        message: ClaudeResponse,
        #[serde(default)]
        index: Option<usize>,
    },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: ClaudeContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        index: usize,
        delta: ClaudeDelta,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop {
        index: usize,
    },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: ClaudeMessageDelta,
        #[serde(default)]
        usage: Option<ClaudeUsage>,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: ClaudeErrorDetail },
}

/// Claude Content Delta (for streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDelta {
    #[serde(rename = "type")]
    pub delta_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// Claude Message Delta (for streaming)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessageDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// Claude Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeError {
    pub error: ClaudeErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

impl ClaudeResponse {
    /// Extract text content from response
    pub fn get_text(&self) -> Option<String> {
        for block in &self.content {
            if block.block_type == "text" {
                if let Some(text) = &block.text {
                    return Some(text.clone());
                }
            }
        }
        None
    }

    /// Extract tool use blocks from response
    pub fn get_tool_uses(&self) -> Vec<&ClaudeContentBlock> {
        self.content
            .iter()
            .filter(|b| b.block_type == "tool_use")
            .collect()
    }
}