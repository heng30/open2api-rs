use crate::{
    backend::{BackendClient, BackendError},
    config::AppConfig,
    models::openai::{OpenAIError, OpenAIErrorDetail, OpenAIRequest},
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

pub fn create_router(state: AppState) -> AxumRouter {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    AxumRouter::new()
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/health", get(handle_health))
        .layer(cors)
        .with_state(state)
}

async fn handle_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    if !check_auth(&headers, &state.config.auth_keys) {
        return auth_error_response();
    }

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
        handle_stream_request(state, openai_request).await
    } else {
        handle_non_stream_request(&state, openai_request).await
    }
}

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

fn check_auth(headers: &HeaderMap, allowed_keys: &[String]) -> bool {
    if allowed_keys.is_empty() {
        return true;
    }

    let auth_header = match headers.get("authorization").and_then(|h| h.to_str().ok()) {
        Some(h) => h,
        None => return false,
    };

    let token = match auth_header.strip_prefix("Bearer ") {
        Some(t) => t.trim(),
        None => return false,
    };

    allowed_keys.iter().any(|k| k == token)
}

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
