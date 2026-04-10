# open2api

Convert Claude API to OpenAI compatible API.

## Overview

`open2api` is a Rust-based proxy service that provides an OpenAI-compatible API interface for Claude (Anthropic) models. This allows you to use any OpenAI-compatible client or tool with Claude models.

### Features

- OpenAI-compatible `/v1/chat/completions` endpoint
- OpenAI-compatible `/v1/models` endpoint
- Health check endpoint at `/health`
- Support for streaming and non-streaming responses
- CORS enabled for browser-based clients
- Coding Agent backend support for Alibaba Cloud Bailian
- Frontend authentication with API keys

## Configuration

Configuration is done through environment variables. You can set them directly or use a `.env` file.

### Environment Variables

| Variable | Required | Description | Default |
|----------|----------|-------------|---------|
| `OPEN2API_HOST` | No | Server host address | `0.0.0.0` |
| `OPEN2API_PORT` | No | Server port | `8080` |
| `OPEN2API_BACKEND_URL` | No | Backend API base URL | `https://api.anthropic.com` |
| `OPEN2API_BACKEND_API_KEY` | Yes | Backend API key | - |
| `OPEN2API_MODEL` | No | Default model name | `claude-sonnet-4-6` |
| `OPEN2API_API_KEY` | No | Frontend authentication keys (comma-separated). If not set, no authentication required. | - |

### Alibaba Cloud Bailian Coding Agent

The proxy supports the Alibaba Cloud Bailian Coding Agent API. To use it:

```env
OPEN2API_BACKEND_URL=https://coding.dashscope.aliyuncs.com/apps/anthropic
OPEN2API_BACKEND_API_KEY=sk-xxx
OPEN2API_MODEL=qwen3.5-plus
```

The proxy automatically adds the required headers:
- `anthropic-version: 2023-06-01`
- `anthropic-beta: managed-agents-2026-04-01`

### Example `.env` File

```env
# Server Configuration
OPEN2API_HOST=0.0.0.0
OPEN2API_PORT=8080

# Backend Configuration (Alibaba Cloud Bailian)
OPEN2API_BACKEND_URL=https://coding.dashscope.aliyuncs.com/apps/anthropic
OPEN2API_BACKEND_API_KEY=sk-xxx
OPEN2API_MODEL=qwen3.5-plus

# Frontend Authentication (optional)
# If set, requests must include Authorization: Bearer <key>
OPEN2API_API_KEY=your-secret-key

# Logging
RUST_LOG=info
```

## Supported Models

The following Claude models are exposed:

- `claude-3-opus`
- `claude-3-sonnet`
- `claude-3-haiku`
- `claude-3-5-sonnet`
- `claude-3-5-opus`

For Bailian Coding Agent, use the model names supported by Bailian (e.g., `qwen3.5-plus`).

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/chat/completions` | POST | Chat completions (OpenAI compatible) |
| `/v1/models` | GET | List available models |
| `/health` | GET | Health check with backend status |

## Usage Examples

### Non-Streaming Request

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-key" \
  -d '{
    "model": "qwen3.5-plus",
    "messages": [
      {"role": "user", "content": "Hello, who are you?"}
    ],
    "temperature": 0.7,
    "max_tokens": 100
  }'
```

### Streaming Request

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-key" \
  -d '{
    "model": "qwen3.5-plus",
    "messages": [
      {"role": "user", "content": "Tell me a short story"}
    ],
    "stream": true
  }'
```

### List Models

```bash
curl -H "Authorization: Bearer your-secret-key" http://localhost:8080/v1/models
```

### Health Check

```bash
curl http://localhost:8080/health
```

### Multi-turn Conversation

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-key" \
  -d '{
    "model": "qwen3.5-plus",
    "messages": [
      {"role": "user", "content": "What is Rust?"},
      {"role": "assistant", "content": "Rust is a programming language..."},
      {"role": "user", "content": "What are its main features?"}
    ]
  }'
```

## Running

### Build and Run

```bash
cargo build --release
cargo run --release
```

### With Custom Port

```bash
OPEN2API_PORT=3000 cargo run --release
```

### With Environment File

Create a `.env` file in the project root and run:

```bash
cargo run --release
```

## License

MIT