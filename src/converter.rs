use crate::models::claude::{
    ClaudeContent, ClaudeContentBlock, ClaudeMessage, ClaudeRequest,
    ClaudeResponse, ClaudeStreamEvent, ClaudeTool, ClaudeToolChoice, ClaudeToolChoiceTool,
    ClaudeUsage,
};
use crate::models::openai::{
    OpenAIChoice, OpenAIContent, OpenAIDelta, OpenAIFunctionCall,
    OpenAIFunctionCallDelta, OpenAIMessage, OpenAIRequest, OpenAIResponse, OpenAIStreamChunk,
    OpenAIStreamChoice, OpenAITool, OpenAIToolCall, OpenAIToolCallDelta, OpenAIUsage,
};
use chrono::Utc;

/// Default max_tokens for Claude requests if not specified
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Convert OpenAI request to Claude request
pub fn openai_to_claude(openai_request: &OpenAIRequest) -> ClaudeRequest {
    let mut messages: Vec<ClaudeMessage> = Vec::new();
    let mut system_prompt: Option<String> = None;

    // Process messages - extract system prompt and convert to Claude format
    for msg in &openai_request.messages {
        if msg.role == "system" {
            // Claude uses a separate system field
            system_prompt = extract_text_content(&msg.content);
        } else {
            // Convert user/assistant/tool messages
            let claude_msg = convert_message(msg);
            messages.push(claude_msg);
        }
    }

    // Convert tools if present
    let tools = openai_request.tools.as_ref().map(|t| convert_tools(t));

    // Convert tool_choice if present
    let tool_choice = openai_request.tool_choice.as_ref().map(|c| convert_tool_choice(c));

    // Get max_tokens from request or use default
    let max_tokens = openai_request.max_tokens.or(Some(DEFAULT_MAX_TOKENS));

    ClaudeRequest {
        model: openai_request.model.clone(),
        messages,
        max_tokens,
        system: system_prompt,
        stream: openai_request.stream,
        temperature: openai_request.temperature,
        tools,
        tool_choice,
    }
}

/// Extract text from OpenAI content
fn extract_text_content(content: &Option<OpenAIContent>) -> Option<String> {
    match content {
        Some(OpenAIContent::Text(text)) => Some(text.clone()),
        Some(OpenAIContent::Parts(parts)) => {
            let texts: Vec<String> = parts
                .iter()
                .filter(|p| p.content_type == "text")
                .filter_map(|p| p.text.clone())
                .collect();
            if texts.is_empty() {
                None
            } else {
                Some(texts.join("\n"))
            }
        }
        None => None,
    }
}

/// Convert a single OpenAI message to Claude message
fn convert_message(msg: &OpenAIMessage) -> ClaudeMessage {
    let role = convert_role(&msg.role);

    // Handle tool response messages
    if msg.role == "tool" {
        let content_text = extract_text_content(&msg.content).unwrap_or_default();
        let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();

        // Claude tool result format
        return ClaudeMessage {
            role,
            content: ClaudeContent::Blocks(vec![ClaudeContentBlock {
                block_type: "tool_result".to_string(),
                text: Some(content_text),
                id: Some(tool_call_id),
                ..Default::default()
            }]),
        };
    }

    // Convert content
    let content = convert_content(&msg.content, &msg.tool_calls);

    ClaudeMessage { role, content }
}

/// Convert OpenAI role to Claude role
fn convert_role(role: &str) -> String {
    match role {
        "user" => "user",
        "assistant" => "assistant",
        "tool" => "user", // Tool results are from user perspective in Claude
        _ => role,
    }
    .to_string()
}

/// Convert OpenAI content to Claude content
fn convert_content(
    content: &Option<OpenAIContent>,
    tool_calls: &Option<Vec<OpenAIToolCall>>,
) -> ClaudeContent {
    let mut blocks: Vec<ClaudeContentBlock> = Vec::new();

    // Handle text/image content
    if let Some(c) = content {
        match c {
            OpenAIContent::Text(text) => {
                if !text.is_empty() {
                    blocks.push(ClaudeContentBlock {
                        block_type: "text".to_string(),
                        text: Some(text.clone()),
                        ..Default::default()
                    });
                }
            }
            OpenAIContent::Parts(parts) => {
                for part in parts {
                    match part.content_type.as_str() {
                        "text" => {
                            if let Some(text) = &part.text {
                                blocks.push(ClaudeContentBlock {
                                    block_type: "text".to_string(),
                                    text: Some(text.clone()),
                                    ..Default::default()
                                });
                            }
                        }
                        "image_url" => {
                            if let Some(image_url) = &part.image_url {
                                // Convert URL to base64 if it's a data URL
                                if let Some(block) = convert_image_url(image_url) {
                                    blocks.push(block);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Handle tool calls in assistant message
    if let Some(calls) = tool_calls {
        for call in calls {
            blocks.push(ClaudeContentBlock {
                block_type: "tool_use".to_string(),
                id: Some(call.id.clone()),
                name: Some(call.function.name.clone()),
                input: Some(serde_json::from_str(&call.function.arguments).unwrap_or(
                    serde_json::Value::Null,
                )),
                ..Default::default()
            });
        }
    }

    if blocks.is_empty() {
        ClaudeContent::Text(String::new())
    } else {
        ClaudeContent::Blocks(blocks)
    }
}

/// Convert OpenAI image URL to Claude image block
fn convert_image_url(image_url: &crate::models::openai::OpenAIImageUrl) -> Option<ClaudeContentBlock> {
    let url = &image_url.url;

    // Handle data URL (base64)
    if url.starts_with("data:") {
        let parts: Vec<&str> = url.splitn(2, ',').collect();
        if parts.len() == 2 {
            let mime_part = parts[0];
            let data = parts[1];

            // Parse media type from "data:image/png;base64"
            let media_type = mime_part
                .strip_prefix("data:")
                .and_then(|s| s.split(';').next())
                .unwrap_or("image/png");

            return Some(ClaudeContentBlock {
                block_type: "image".to_string(),
                source: Some(crate::models::claude::ClaudeImageSource {
                    source_type: "base64".to_string(),
                    media_type: media_type.to_string(),
                    data: data.to_string(),
                }),
                ..Default::default()
            });
        }
    }

    // For external URLs, we would need to fetch and convert to base64
    // For now, skip external URLs
    None
}

/// Convert OpenAI tools to Claude tools
fn convert_tools(openai_tools: &[OpenAITool]) -> Vec<ClaudeTool> {
    openai_tools
        .iter()
        .filter(|t| t.tool_type == "function")
        .map(|t| ClaudeTool {
            name: t.function.name.clone(),
            description: t.function.description.clone(),
            input_schema: t.function.parameters.clone().unwrap_or(serde_json::json!({
                "type": "object",
                "properties": {}
            })),
        })
        .collect()
}

/// Convert OpenAI tool_choice to Claude tool_choice
fn convert_tool_choice(choice: &crate::models::openai::OpenAIToolChoice) -> ClaudeToolChoice {
    match choice {
        crate::models::openai::OpenAIToolChoice::String(s) => {
            match s.as_str() {
                "auto" => ClaudeToolChoice::Type("auto".to_string()),
                "none" => ClaudeToolChoice::Type("none".to_string()),
                "required" => ClaudeToolChoice::Type("any".to_string()),
                _ => ClaudeToolChoice::Type("auto".to_string()),
            }
        }
        crate::models::openai::OpenAIToolChoice::Object(obj) => {
            ClaudeToolChoice::Tool(ClaudeToolChoiceTool {
                choice_type: "tool".to_string(),
                name: obj.function.name.clone(),
            })
        }
    }
}

/// Convert Claude response to OpenAI response
pub fn claude_to_openai(claude_response: &ClaudeResponse, request_model: &str) -> OpenAIResponse {
    let created = Utc::now().timestamp();

    // Convert content blocks to OpenAI message
    let message = convert_claude_response_message(claude_response);

    // Convert usage
    let usage = claude_response.usage.as_ref().map(convert_usage);

    // Convert stop_reason to finish_reason
    let finish_reason = convert_stop_reason(&claude_response.stop_reason);

    OpenAIResponse {
        id: claude_response.id.clone(),
        object: "chat.completion".to_string(),
        created,
        model: request_model.to_string(),
        choices: vec![OpenAIChoice {
            index: 0,
            message,
            finish_reason,
        }],
        usage,
    }
}

/// Convert Claude response content to OpenAI message
fn convert_claude_response_message(claude_response: &ClaudeResponse) -> OpenAIMessage {
    let mut text_content = String::new();
    let mut tool_calls: Vec<OpenAIToolCall> = Vec::new();

    for block in &claude_response.content {
        match block.block_type.as_str() {
            "text" => {
                if let Some(text) = &block.text {
                    text_content.push_str(text);
                }
            }
            "tool_use" => {
                let arguments = block
                    .input
                    .as_ref()
                    .map(|i| serde_json::to_string(i).unwrap_or_default())
                    .unwrap_or_default();

                tool_calls.push(OpenAIToolCall {
                    id: block.id.clone().unwrap_or_default(),
                    call_type: "function".to_string(),
                    function: OpenAIFunctionCall {
                        name: block.name.clone().unwrap_or_default(),
                        arguments,
                    },
                });
            }
            _ => {}
        }
    }

    OpenAIMessage {
        role: "assistant".to_string(),
        content: if text_content.is_empty() && tool_calls.is_empty() {
            None
        } else if tool_calls.is_empty() {
            Some(OpenAIContent::Text(text_content))
        } else {
            Some(OpenAIContent::Text(text_content))
        },
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
        ..Default::default()
    }
}

/// Convert Claude usage to OpenAI usage
fn convert_usage(claude_usage: &ClaudeUsage) -> OpenAIUsage {
    OpenAIUsage {
        prompt_tokens: claude_usage.input_tokens,
        completion_tokens: claude_usage.output_tokens,
        total_tokens: claude_usage.input_tokens + claude_usage.output_tokens,
    }
}

/// Convert Claude stop_reason to OpenAI finish_reason
fn convert_stop_reason(stop_reason: &Option<String>) -> Option<String> {
    stop_reason.as_ref().map(|r| match r.as_str() {
        "end_turn" => "stop".to_string(),
        "max_tokens" => "length".to_string(),
        "stop_sequence" => "stop".to_string(),
        "tool_use" => "tool_calls".to_string(),
        _ => "stop".to_string(),
    })
}

/// Convert Claude streaming event to OpenAI stream chunk(s)
pub fn claude_stream_to_openai(
    event: &ClaudeStreamEvent,
    response_id: &str,
    request_model: &str,
    created: i64,
) -> Vec<String> {
    match event {
        ClaudeStreamEvent::MessageStart { message: _, .. } => {
            // Initialize with role
            let chunk = OpenAIStreamChunk {
                id: response_id.to_string(),
                object: "chat.completion.chunk".to_string(),
                created,
                model: request_model.to_string(),
                choices: vec![OpenAIStreamChoice {
                    index: 0,
                    delta: OpenAIDelta {
                        role: Some("assistant".to_string()),
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason: None,
                }],
            };
            vec![chunk.to_sse()]
        }
        ClaudeStreamEvent::ContentBlockStart { index, content_block } => {
            if content_block.block_type == "tool_use" {
                // Start of a tool call
                let chunk = OpenAIStreamChunk {
                    id: response_id.to_string(),
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: request_model.to_string(),
                    choices: vec![OpenAIStreamChoice {
                        index: 0,
                        delta: OpenAIDelta {
                            role: None,
                            content: None,
                            tool_calls: Some(vec![OpenAIToolCallDelta {
                                index: *index,
                                id: content_block.id.clone(),
                                function: Some(OpenAIFunctionCallDelta {
                                    name: content_block.name.clone(),
                                    arguments: None,
                                }),
                            }]),
                        },
                        finish_reason: None,
                    }],
                };
                vec![chunk.to_sse()]
            } else {
                // Text content starts - no action needed
                vec![]
            }
        }
        ClaudeStreamEvent::ContentBlockDelta { index, delta } => {
            match delta.delta_type.as_str() {
                "text_delta" => {
                    // Text delta
                    if let Some(text) = &delta.text {
                        let chunk = OpenAIStreamChunk::new_text(
                            response_id,
                            request_model,
                            created,
                            Some(text.clone()),
                            None,
                        );
                        vec![chunk.to_sse()]
                    } else {
                        vec![]
                    }
                }
                "input_json_delta" => {
                    // Tool input delta
                    if let Some(partial_json) = &delta.partial_json {
                        let chunk = OpenAIStreamChunk {
                            id: response_id.to_string(),
                            object: "chat.completion.chunk".to_string(),
                            created,
                            model: request_model.to_string(),
                            choices: vec![OpenAIStreamChoice {
                                index: 0,
                                delta: OpenAIDelta {
                                    role: None,
                                    content: None,
                                    tool_calls: Some(vec![OpenAIToolCallDelta {
                                        index: *index,
                                        id: None,
                                        function: Some(OpenAIFunctionCallDelta {
                                            name: None,
                                            arguments: Some(partial_json.clone()),
                                        }),
                                    }]),
                                },
                                finish_reason: None,
                            }],
                        };
                        vec![chunk.to_sse()]
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            }
        }
        ClaudeStreamEvent::ContentBlockStop { .. } => {
            // Content block end - no action needed for OpenAI
            vec![]
        }
        ClaudeStreamEvent::MessageDelta { delta, .. } => {
            // Message delta - may include stop_reason
            if let Some(stop_reason) = &delta.stop_reason {
                let finish_reason = convert_stop_reason(&Some(stop_reason.clone()));
                let chunk = OpenAIStreamChunk {
                    id: response_id.to_string(),
                    object: "chat.completion.chunk".to_string(),
                    created,
                    model: request_model.to_string(),
                    choices: vec![OpenAIStreamChoice {
                        index: 0,
                        delta: OpenAIDelta {
                            role: None,
                            content: None,
                            tool_calls: None,
                        },
                        finish_reason,
                    }],
                };
                vec![chunk.to_sse()]
            } else {
                vec![]
            }
        }
        ClaudeStreamEvent::MessageStop => {
            // End of message - send [DONE]
            vec![OpenAIStreamChunk::done_marker()]
        }
        ClaudeStreamEvent::Ping => vec![],
        ClaudeStreamEvent::Error { error } => {
            // Error - send as OpenAI error
            let error_resp = crate::models::openai::OpenAIError {
                error: crate::models::openai::OpenAIErrorDetail {
                    message: error.message.clone(),
                    error_type: error.error_type.clone(),
                    param: None,
                    code: None,
                },
            };
            vec![format!(
                "data: {}\n\n",
                serde_json::to_string(&error_resp).unwrap_or_default()
            )]
        }
    }
}