use crate::models::{
    claude::{
        ClaudeContent, ClaudeContentBlock, ClaudeImageSource, ClaudeMessage, ClaudeRequest,
        ClaudeResponse, ClaudeStreamEvent, ClaudeTool, ClaudeToolChoice, ClaudeToolChoiceTool,
        ClaudeUsage,
    },
    openai::{
        OpenAIChoice, OpenAIContent, OpenAIDelta, OpenAIError, OpenAIErrorDetail,
        OpenAIFunctionCall, OpenAIFunctionCallDelta, OpenAIImageUrl, OpenAIMessage, OpenAIRequest,
        OpenAIResponse, OpenAIStreamChoice, OpenAIStreamChunk, OpenAITool, OpenAIToolCall,
        OpenAIToolCallDelta, OpenAIToolChoice, OpenAIUsage,
    },
};

pub fn openai_to_claude(openai_request: &OpenAIRequest, default_max_tokens: u32) -> ClaudeRequest {
    let mut messages: Vec<ClaudeMessage> = Vec::new();
    let mut system_prompt: Option<String> = None;

    for msg in &openai_request.messages {
        if msg.role == "system" {
            system_prompt = extract_text_content(&msg.content);
        } else {
            messages.push(convert_message(msg));
        }
    }

    let max_tokens = openai_request.max_tokens.or(Some(default_max_tokens));
    let tools = openai_request.tools.as_ref().map(|t| convert_tools(t));
    let tool_choice = openai_request
        .tool_choice
        .as_ref()
        .map(|c| convert_tool_choice(c));

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

fn convert_message(msg: &OpenAIMessage) -> ClaudeMessage {
    let role = convert_role(&msg.role);

    if msg.role == "tool" {
        let content_text = extract_text_content(&msg.content).unwrap_or_default();
        let tool_call_id = msg.tool_call_id.clone().unwrap_or_default();

        return ClaudeMessage {
            role,
            content: ClaudeContent::Blocks(vec![ClaudeContentBlock {
                block_type: "tool_result".to_string(),
                id: Some(tool_call_id),
                text: Some(content_text),
                ..Default::default()
            }]),
        };
    }

    ClaudeMessage {
        role,
        content: convert_content(&msg.content, &msg.tool_calls),
    }
}

fn convert_role(role: &str) -> String {
    match role {
        "user" => "user",
        "assistant" => "assistant",
        "tool" => "user", // Tool results are from user perspective in Claude
        _ => role,
    }
    .to_string()
}

fn convert_content(
    content: &Option<OpenAIContent>,
    tool_calls: &Option<Vec<OpenAIToolCall>>,
) -> ClaudeContent {
    let mut blocks: Vec<ClaudeContentBlock> = Vec::new();

    if let Some(c) = content {
        match c {
            OpenAIContent::Text(text) => {
                if tool_calls.is_none() || tool_calls.as_ref().unwrap().is_empty() {
                    return ClaudeContent::Text(text.clone());
                }
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
                            if let Some(image_url) = &part.image_url
                                && let Some(block) = convert_image_url(image_url)
                            {
                                blocks.push(block);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if let Some(calls) = tool_calls {
        for call in calls {
            blocks.push(ClaudeContentBlock {
                block_type: "tool_use".to_string(),
                id: Some(call.id.clone()),
                name: Some(call.function.name.clone()),
                input: Some(
                    serde_json::from_str(&call.function.arguments)
                        .unwrap_or(serde_json::Value::Null),
                ),
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

fn convert_image_url(image_url: &OpenAIImageUrl) -> Option<ClaudeContentBlock> {
    let url = &image_url.url;

    // Parse media type from "data:image/png;base64"
    if url.starts_with("data:") {
        let parts: Vec<&str> = url.splitn(2, ',').collect();
        if parts.len() == 2 {
            let mime_part = parts[0];
            let data = parts[1];

            let media_type = mime_part
                .strip_prefix("data:")
                .and_then(|s| s.split(';').next())
                .unwrap_or("image/png");

            return Some(ClaudeContentBlock {
                block_type: "image".to_string(),
                source: Some(ClaudeImageSource {
                    source_type: "base64".to_string(),
                    media_type: media_type.to_string(),
                    data: data.to_string(),
                }),
                ..Default::default()
            });
        }
    }

    None
}

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

fn convert_tool_choice(choice: &OpenAIToolChoice) -> ClaudeToolChoice {
    match choice {
        OpenAIToolChoice::String(s) => match s.as_str() {
            "auto" => ClaudeToolChoice::Type("auto".to_string()),
            "none" => ClaudeToolChoice::Type("none".to_string()),
            "required" => ClaudeToolChoice::Type("any".to_string()),
            _ => ClaudeToolChoice::Type("auto".to_string()),
        },
        OpenAIToolChoice::Object(obj) => ClaudeToolChoice::Tool(ClaudeToolChoiceTool {
            choice_type: "tool".to_string(),
            name: obj.function.name.clone(),
        }),
    }
}

pub fn claude_to_openai(claude_response: &ClaudeResponse, request_model: &str) -> OpenAIResponse {
    let created = chrono::Utc::now().timestamp();
    let message = convert_claude_response_message(claude_response);
    let usage = claude_response.usage.as_ref().map(convert_usage);
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

fn convert_usage(claude_usage: &ClaudeUsage) -> OpenAIUsage {
    OpenAIUsage {
        prompt_tokens: claude_usage.input_tokens,
        completion_tokens: claude_usage.output_tokens,
        total_tokens: claude_usage.input_tokens + claude_usage.output_tokens,
    }
}

fn convert_stop_reason(stop_reason: &Option<String>) -> Option<String> {
    stop_reason.as_ref().map(|r| match r.as_str() {
        "end_turn" => "stop".to_string(),
        "max_tokens" => "length".to_string(),
        "stop_sequence" => "stop".to_string(),
        "tool_use" => "tool_calls".to_string(),
        _ => "stop".to_string(),
    })
}

pub fn claude_stream_to_openai(
    event: &ClaudeStreamEvent,
    response_id: &str,
    request_model: &str,
    created: i64,
) -> Vec<String> {
    match event {
        ClaudeStreamEvent::MessageStart { message: _, .. } => {
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
        ClaudeStreamEvent::ContentBlockStart {
            index,
            content_block,
        } => {
            if content_block.block_type == "tool_use" {
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
                vec![]
            }
        }
        ClaudeStreamEvent::ContentBlockDelta { index, delta } => match delta.delta_type.as_str() {
            "text_delta" => {
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
        },
        ClaudeStreamEvent::ContentBlockStop { .. } => vec![],
        ClaudeStreamEvent::MessageDelta { delta, .. } => {
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
        ClaudeStreamEvent::MessageStop => vec![OpenAIStreamChunk::done_marker()],
        ClaudeStreamEvent::Ping => vec![],
        ClaudeStreamEvent::Error { error } => {
            let error_resp = OpenAIError {
                error: OpenAIErrorDetail {
                    message: error.message.clone(),
                    error_type: error.error_type.clone(),
                    param: None,
                    code: None,
                },
            };
            vec![serde_json::to_string(&error_resp).unwrap_or_default()]
        }
    }
}
