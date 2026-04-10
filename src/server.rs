use crate::backend::{BackendClient, BackendError};
use crate::config::AppConfig;
use crate::models::openai::{OpenAIError, OpenAIErrorDetail, OpenAIModel, OpenAIModelsResponse, OpenAIRequest};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response, Sse},
    routing::{get, post},
    Router as AxumRouter,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub client: Arc<BackendClient>,
    pub config: Arc<AppConfig>,
}

impl AppState {
    pub fn new(client: BackendClient, config: AppConfig) -> Self {
        AppState {
            client: Arc::new(client),
            config: Arc::new(config),
        }
    }
}

/// Create the Axum router (returns Router<()> for use with into_make_service_with_connect_info)
pub fn create_router(state: AppState) -> AxumRouter {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    AxumRouter::new()
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/models", get(handle_models))
        .route("/health", get(handle_health))
        .layer(cors)
        .with_state(state)
}

/// Handle chat completions endpoint
async fn handle_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    // Parse the request body
    let body = match axum::body::to_bytes(request.into_body(), 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                &e.to_string(),
            );
        }
    };

    let openai_request: OpenAIRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_error",
                &format!("Failed to parse request: {}", e),
            );
        }
    };

    // Get client info for routing
    // Use X-Forwarded-For or X-Real-IP header if available (reverse proxy setup)
    // Otherwise, use a placeholder (in production, you'd use ConnectInfo with proper setup)
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("unknown").trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    tracing::debug!(
        "Chat request: model={}, stream={}, client_ip={}, user_agent={}",
        openai_request.model,
        openai_request.stream,
        client_ip,
        user_agent
    );

    if openai_request.stream {
        // Handle streaming request - pass owned strings
        handle_stream_request(state, openai_request, client_ip, user_agent).await
    } else {
        // Handle non-streaming request
        handle_non_stream_request(&state, openai_request, &client_ip, &user_agent).await
    }
}

/// Handle non-streaming chat completion request
async fn handle_non_stream_request(
    state: &AppState,
    request: OpenAIRequest,
    client_ip: &str,
    user_agent: &str,
) -> Response {
    match state.client.chat_completion(request, client_ip, user_agent).await {
        Ok(response) => Json(response).into_response(),
        Err(BackendError::ApiError(status, body)) => {
            tracing::error!("API error {}: {}", status, body);
            error_response(
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                "api_error",
                &body,
            )
        }
        Err(e) => {
            tracing::error!("Backend error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", &e.to_string())
        }
    }
}

/// Handle streaming chat completion request
async fn handle_stream_request(
    state: AppState,
    request: OpenAIRequest,
    client_ip: String,
    user_agent: String,
) -> Response {
    match state.client.chat_completion_stream(request, &client_ip, &user_agent).await {
        Ok(stream) => Sse::new(stream)
            .keep_alive(axum::response::sse::KeepAlive::new())
            .into_response(),
        Err(BackendError::ApiError(status, body)) => {
            tracing::error!("API stream error {}: {}", status, body);
            error_response(
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                "api_error",
                &body,
            )
        }
        Err(e) => {
            tracing::error!("Backend stream error: {}", e);
            error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", &e.to_string())
        }
    }
}

/// Handle models endpoint
async fn handle_models(_state: State<AppState>) -> Response {
    // Return a list of supported models
    let models: Vec<OpenAIModel> = vec![
        OpenAIModel {
            id: "claude-3-opus".to_string(),
            object: "model".to_string(),
            created: 1700000000,
            owned_by: "anthropic".to_string(),
        },
        OpenAIModel {
            id: "claude-3-sonnet".to_string(),
            object: "model".to_string(),
            created: 1700000000,
            owned_by: "anthropic".to_string(),
        },
        OpenAIModel {
            id: "claude-3-haiku".to_string(),
            object: "model".to_string(),
            created: 1700000000,
            owned_by: "anthropic".to_string(),
        },
        OpenAIModel {
            id: "claude-3-5-sonnet".to_string(),
            object: "model".to_string(),
            created: 1700000000,
            owned_by: "anthropic".to_string(),
        },
        OpenAIModel {
            id: "claude-3-5-opus".to_string(),
            object: "model".to_string(),
            created: 1700000000,
            owned_by: "anthropic".to_string(),
        },
    ];

    Json(OpenAIModelsResponse {
        object: "list".to_string(),
        data: models,
    }).into_response()
}

/// Handle health check endpoint
async fn handle_health(State(state): State<AppState>) -> Response {
    let health_summary = state.client.router().pool().get_health_summary().await;

    Json(serde_json::json!({
        "status": "ok",
        "backends": health_summary
    })).into_response()
}

/// Create an error response
fn error_response(status: StatusCode, error_type: &str, message: &str) -> Response {
    let error = OpenAIError {
        error: OpenAIErrorDetail {
            message: message.to_string(),
            error_type: error_type.to_string(),
            param: None,
            code: Some(status.as_u16().to_string()),
        },
    };

    (status, Json(error)).into_response()
}