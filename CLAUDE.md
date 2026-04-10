# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Run Commands

```bash
# Build release binary
cargo build --release

# Run server
cargo run --release

# Run with environment file
cargo run --release  # reads .env automatically
```

## Example Programs

```bash
# Stream demo (requires running server)
cargo run --example stream_demo

# Non-stream demo
cargo run --example non_stream_demo
```

## Environment Configuration

Required environment variables (set in `.env` or directly):
- `OPEN2API_BACKEND_API_KEY`: Backend API key (required)
- `OPEN2API_BACKEND_URL`: Backend URL (default: `https://api.anthropic.com`)
- `OPEN2API_MODELS`: Supported model names, comma-separated (default: `claude-sonnet-4-6`)
- `OPEN2API_MODEL`: (Deprecated) Single model name, fallback if `OPEN2API_MODELS` not set
- `OPEN2API_PORT`: Server port (default: `8080`)
- `OPEN2API_API_KEYS`: Frontend auth keys, comma-separated (optional)

For Alibaba Cloud Bailian:
```env
OPEN2API_BACKEND_URL=https://coding.dashscope.aliyuncs.com/apps/anthropic
OPEN2API_BACKEND_API_KEY=sk-xxx
OPEN2API_MODELS=qwen3.5-plus,claude-sonnet-4-6
```

## Project Structure

This is a Rust workspace with two members:
- Main crate (`open2api`): OpenAI-compatible API proxy server
- `lib/bot`: Helper library for chat client functionality

Key source modules in `src/`:
- `main.rs`: Entry point, initializes server
- `config.rs`: `AppConfig` struct with environment variable parsing
- `backend.rs`: `BackendClient` for backend API communication with streaming support
- `server.rs`: Axum routes (`/v1/chat/completions`, `/v1/models`, `/health`) and handlers
- `converter.rs`: Bidirectional conversion between OpenAI and Claude request/response formats
- `models/openai.rs`: OpenAI-compatible request/response structs
- `models/claude.rs`: Claude API request/response structs

## Architecture Overview

The proxy converts OpenAI-format requests to Claude API format, calls the backend, then converts responses back to OpenAI format. Both streaming and non-streaming modes are supported.

Request flow:
1. Client sends OpenAI-format request to `/v1/chat/completions`
2. `server.rs` handler validates auth and parses request
3. `converter.rs::openai_to_claude()` transforms request format
4. `backend.rs::BackendClient` calls backend API (streaming or non-streaming)
5. Response converted via `converter.rs::claude_to_openai()` or `claude_stream_to_openai()`
6. OpenAI-format response returned to client

The `ClaudeToOpenAIStream` struct in `backend.rs` implements a custom `Stream` trait for SSE streaming conversion.

## Key Dependencies

- `axum`: HTTP server framework
- `reqwest`: HTTP client for backend API calls
- `tower-http`: CORS middleware
- `tokio`: Async runtime
- `serde/serde_json`: JSON serialization