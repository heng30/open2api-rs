use crate::config::AppConfig;
use crate::converter::{claude_stream_to_openai, claude_to_openai, openai_to_claude};
use crate::models::claude::{ClaudeResponse, ClaudeStreamEvent};
use crate::models::openai::{OpenAIRequest, OpenAIResponse, OpenAIStreamChunk};
use axum::response::sse::Event;
use futures::stream::Stream;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio_stream::StreamExt;

/// Backend client for making Claude API requests with Coding Agent support
pub struct BackendClient {
    client: reqwest::Client,
    config: Arc<AppConfig>,
}

impl BackendClient {
    /// Create a new backend client
    pub fn new(config: AppConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .http1_only()  // Force HTTP/1.1
            .build()
            .expect("Failed to create HTTP client");

        BackendClient {
            client,
            config: Arc::new(config),
        }
    }

    /// Get the config reference
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Make a chat completion request
    pub async fn chat_completion(
        &self,
        request: OpenAIRequest,
    ) -> Result<OpenAIResponse, BackendError> {
        tracing::info!(
            "Sending request to Coding Agent backend: {} (model: {})",
            self.config.base_url,
            self.config.model
        );

        // Convert OpenAI request to Claude request
        let claude_request = openai_to_claude(&request);

        // Make the request with Coding Agent header
        let url = format!("{}{}", self.config.base_url, "/v1/messages");

        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .header(AUTHORIZATION, format!("Bearer {}", self.config.api_key))
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "managed-agents-2026-04-01")
            .header("User-Agent", "curl/8.18.0")
            .json(&claude_request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await?;
            tracing::error!("Backend error: status={}, body={}", status, error_body);
            return Err(BackendError::ApiError(status.as_u16(), error_body));
        }

        // Parse Claude response
        let claude_response: ClaudeResponse = response.json().await?;

        // Convert to OpenAI response
        let openai_response = claude_to_openai(&claude_response, &request.model);

        Ok(openai_response)
    }

    /// Make a streaming chat completion request
    pub async fn chat_completion_stream(
        &self,
        request: OpenAIRequest,
    ) -> Result<impl Stream<Item = Result<Event, Infallible>> + Send + use<>, BackendError> {
        tracing::info!(
            "Sending stream request to Coding Agent backend: {} (model: {})",
            self.config.base_url,
            self.config.model
        );

        // Convert OpenAI request to Claude request
        let claude_request = openai_to_claude(&request);
        let model = request.model.clone();

        // Make the streaming request with Coding Agent header
        let url = format!("{}{}", self.config.base_url, "/v1/messages");

        let response = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream")
            .header(AUTHORIZATION, format!("Bearer {}", self.config.api_key))
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "managed-agents-2026-04-01")
            .header("User-Agent", "curl/8.18.0")
            .json(&claude_request)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let error_body = response.text().await?;
            tracing::error!("Backend stream error: status={}, body={}", status, error_body);
            return Err(BackendError::ApiError(status.as_u16(), error_body));
        }

        // Create streaming response converter
        let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4().simple());
        let created = chrono::Utc::now().timestamp();

        // Convert Claude SSE stream to OpenAI SSE stream
        let stream = ClaudeToOpenAIStream::new(response, response_id, model, created);

        Ok(stream.map(|s| Ok(Event::default().data(s))))
    }
}

/// Error type for backend operations
#[derive(Debug)]
pub enum BackendError {
    HttpError(reqwest::Error),
    ApiError(u16, String),
    ParseError(String),
}

impl From<reqwest::Error> for BackendError {
    fn from(e: reqwest::Error) -> Self {
        BackendError::HttpError(e)
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::HttpError(e) => write!(f, "HTTP error: {}", e),
            BackendError::ApiError(status, body) => write!(f, "API error {}: {}", status, body),
            BackendError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for BackendError {}

/// Stream that converts Claude SSE events to OpenAI SSE format
pub struct ClaudeToOpenAIStream {
    inner: Pin<Box<dyn tokio_stream::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    buffer: String,
    response_id: String,
    model: String,
    created: i64,
    is_done: bool,
}

impl ClaudeToOpenAIStream {
    pub fn new(
        response: reqwest::Response,
        response_id: String,
        model: String,
        created: i64,
    ) -> Self {
        ClaudeToOpenAIStream {
            inner: Box::pin(response.bytes_stream()),
            buffer: String::new(),
            response_id,
            model,
            created,
            is_done: false,
        }
    }

    fn process_buffer(&mut self) -> Vec<String> {
        let mut outputs: Vec<String> = Vec::new();

        while let Some(event_end) = self.buffer.find("\n\n") {
            let event_data = self.buffer[..event_end].to_string();
            self.buffer = self.buffer[event_end + 2..].to_string();

            if event_data.is_empty() {
                continue;
            }

            // Parse SSE event
            let mut data: Option<String> = None;

            for line in event_data.lines() {
                if line.starts_with("data:") {
                    data = Some(line[5..].trim().to_string());
                }
            }

            if let Some(data_str) = data {
                // Parse Claude event
                if let Ok(claude_event) = serde_json::from_str::<ClaudeStreamEvent>(&data_str) {
                    let chunks = claude_stream_to_openai(
                        &claude_event,
                        &self.response_id,
                        &self.model,
                        self.created,
                    );

                    // Check if message_stop
                    if matches!(claude_event, ClaudeStreamEvent::MessageStop) {
                        self.is_done = true;
                    }

                    outputs.extend(chunks);
                } else {
                    tracing::warn!("Failed to parse Claude stream event: {}", data_str);
                }
            }
        }

        outputs
    }
}

impl Stream for ClaudeToOpenAIStream {
    type Item = String;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.is_done && self.buffer.is_empty() {
            return Poll::Ready(None);
        }

        // First, try to get more output from existing buffer
        let outputs = self.process_buffer();
        if !outputs.is_empty() {
            return Poll::Ready(Some(outputs.join("")));
        }

        // Poll for more bytes
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                let text = String::from_utf8_lossy(&bytes);
                self.buffer.push_str(&text);

                let outputs = self.process_buffer();
                if !outputs.is_empty() {
                    Poll::Ready(Some(outputs.join("")))
                } else {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Ready(Some(Err(e))) => {
                tracing::error!("Stream error: {}", e);
                Poll::Ready(None)
            }
            Poll::Ready(None) => {
                // Stream ended, process remaining buffer
                let outputs = self.process_buffer();
                if !outputs.is_empty() {
                    Poll::Ready(Some(outputs.join("")))
                } else {
                    // Send [DONE] if not already sent
                    if !self.is_done {
                        self.is_done = true;
                        Poll::Ready(Some(OpenAIStreamChunk::done_marker()))
                    } else {
                        Poll::Ready(None)
                    }
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}