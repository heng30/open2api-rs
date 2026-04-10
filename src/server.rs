use crate::backend::{BackendClient, BackendError};
use crate::config::AppConfig;
use crate::models::openai::{
    OpenAIError, OpenAIErrorDetail, OpenAIModel, OpenAIModelsResponse, OpenAIRequest,
};
use axum::{
    Router as AxumRouter,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response, Sse},
    routing::{get, post},
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
    // Check authentication if API keys are configured
    if !check_auth(&headers, &state.config.auth_keys) {
        return auth_error_response();
    }

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

    tracing::debug!(
        "Chat request: model={}, stream={}",
        openai_request.model,
        openai_request.stream
    );

    // Validate model name
    if openai_request.model != state.config.model {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            &format!(
                "Model '{}' is not supported. This server only supports '{}'",
                openai_request.model, state.config.model
            ),
        );
    }

    if openai_request.stream {
        // Handle streaming request
        handle_stream_request(state, openai_request).await
    } else {
        // Handle non-streaming request
        handle_non_stream_request(&state, openai_request).await
    }
}

/// Handle non-streaming chat completion request
async fn handle_non_stream_request(state: &AppState, request: OpenAIRequest) -> Response {
    match state.client.chat_completion(request).await {
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
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                &e.to_string(),
            )
        }
    }
}

/// Handle streaming chat completion request
async fn handle_stream_request(state: AppState, request: OpenAIRequest) -> Response {
    match state.client.chat_completion_stream(request).await {
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
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                &e.to_string(),
            )
        }
    }
}

/// Handle models endpoint
async fn handle_models(State(state): State<AppState>, headers: HeaderMap) -> Response {
    // Check authentication if API keys are configured
    if !check_auth(&headers, &state.config.auth_keys) {
        return auth_error_response();
    }

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
    })
    .into_response()
}

/// Handle health check endpoint
async fn handle_health(State(state): State<AppState>) -> Response {
    Json(serde_json::json!({
        "status": "ok",
        "backend": {
            "base_url": state.config.base_url,
            "model": state.config.model
        }
    }))
    .into_response()
}

/// Check if the request has valid authentication
fn check_auth(headers: &HeaderMap, allowed_keys: &[String]) -> bool {
    // If no keys configured, allow all requests
    if allowed_keys.is_empty() {
        return true;
    }

    // Extract Bearer token from Authorization header
    let auth_header = match headers.get("authorization").and_then(|h| h.to_str().ok()) {
        Some(h) => h,
        None => return false,
    };

    // Parse "Bearer <token>"
    let token = match auth_header.strip_prefix("Bearer ") {
        Some(t) => t.trim(),
        None => return false,
    };

    // Check if token matches any allowed key
    allowed_keys.iter().any(|k| k == token)
}

/// Create an authentication error response
fn auth_error_response() -> Response {
    let error = OpenAIError {
        error: OpenAIErrorDetail {
            message: "Invalid or missing API key".to_string(),
            error_type: "authentication_error".to_string(),
            param: None,
            code: Some("401".to_string()),
        },
    };

    (StatusCode::UNAUTHORIZED, Json(error)).into_response()
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
