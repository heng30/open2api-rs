# open2api

将 Claude API 转换为 OpenAI 兼容的 API。

## 概述

`open2api` 是一个基于 Rust 的代理服务，为 Claude（Anthropic）模型提供 OpenAI 兼容的 API 接口。这使得您可以使用任何 OpenAI 兼容的客户端或工具来调用 Claude 模型。

### 特性

- OpenAI 兼容的 `/v1/chat/completions` 端点
- OpenAI 兼容的 `/v1/models` 端点
- `/health` 健康检查端点
- 支持流式和非流式响应
- 启用 CORS，支持浏览器客户端
- 支持阿里云百炼 Coding Agent 后端
- 前端 API 密钥认证

## 配置

通过环境变量进行配置。您可以直接设置环境变量，或使用 `.env` 文件。

### 环境变量

| 变量 | 必填 | 说明 | 默认值 |
|------|------|------|--------|
| `OPEN2API_HOST` | 否 | 服务器主机地址 | `0.0.0.0` |
| `OPEN2API_PORT` | 否 | 服务器端口 | `8080` |
| `OPEN2API_BACKEND_URL` | 否 | 后端 API 基础 URL | `https://api.anthropic.com` |
| `OPEN2API_BACKEND_API_KEY` | 是 | 后端 API 密钥 | - |
| `OPEN2API_MODEL` | 否 | 默认模型名称 | `claude-sonnet-4-6` |
| `OPEN2API_API_KEY` | 否 | 前端认证密钥（逗号分隔支持多个）。若不设置，则无需认证。 | - |

### 阿里云百炼 Coding Agent

代理支持阿里云百炼 Coding Agent API。使用方法：

```env
OPEN2API_BACKEND_URL=https://coding.dashscope.aliyuncs.com/apps/anthropic
OPEN2API_BACKEND_API_KEY=sk-xxx
OPEN2API_MODEL=qwen3.5-plus
```

代理会自动添加所需的请求头：
- `anthropic-version: 2023-06-01`
- `anthropic-beta: managed-agents-2026-04-01`

### `.env` 文件示例

```env
# 服务器配置
OPEN2API_HOST=0.0.0.0
OPEN2API_PORT=8080

# 后端配置（阿里云百炼）
OPEN2API_BACKEND_URL=https://coding.dashscope.aliyuncs.com/apps/anthropic
OPEN2API_BACKEND_API_KEY=sk-xxx
OPEN2API_MODEL=qwen3.5-plus

# 前端认证（可选）
# 如果设置，请求需要包含 Authorization: Bearer <密钥>
OPEN2API_API_KEY=your-secret-key

# 日志配置
RUST_LOG=info
```

## 支持的模型

以下 Claude 模型已暴露：

- `claude-3-opus`
- `claude-3-sonnet`
- `claude-3-haiku`
- `claude-3-5-sonnet`
- `claude-3-5-opus`

对于百炼 Coding Agent，请使用百炼支持的模型名称（如 `qwen3.5-plus`）。

## API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/v1/chat/completions` | POST | 聊天补全（OpenAI 兼容） |
| `/v1/models` | GET | 获取可用模型列表 |
| `/health` | GET | 健康检查（含后端状态） |

## 使用示例

### 非流式请求

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-key" \
  -d '{
    "model": "qwen3.5-plus",
    "messages": [
      {"role": "user", "content": "你好，你是谁？"}
    ],
    "temperature": 0.7,
    "max_tokens": 100
  }'
```

### 流式请求

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-key" \
  -d '{
    "model": "qwen3.5-plus",
    "messages": [
      {"role": "user", "content": "给我讲一个小故事"}
    ],
    "stream": true
  }'
```

### 获取模型列表

```bash
curl -H "Authorization: Bearer your-secret-key" http://localhost:8080/v1/models
```

### 健康检查

```bash
curl http://localhost:8080/health
```

### 多轮对话

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-secret-key" \
  -d '{
    "model": "qwen3.5-plus",
    "messages": [
      {"role": "user", "content": "Rust 是什么？"},
      {"role": "assistant", "content": "Rust 是一种编程语言..."},
      {"role": "user", "content": "它有哪些主要特性？"}
    ]
  }'
```

## 运行

### 构建并运行

```bash
cargo build --release
cargo run --release
```

### 使用自定义端口

```bash
OPEN2API_PORT=3000 cargo run --release
```

### 使用环境文件

在项目根目录创建 `.env` 文件，然后运行：

```bash
cargo run --release
```

## 许可证

MIT