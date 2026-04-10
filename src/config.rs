use anyhow::Result;
use std::env;

/// Application configuration for Coding Agent backend
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// API keys for frontend authentication (required to access the proxy)
    pub auth_keys: Vec<String>,
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let host = env::var("OPEN2API_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string());

        let port = env::var("OPEN2API_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let base_url = env::var("OPEN2API_BACKEND_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

        let api_key = env::var("OPEN2API_BACKEND_API_KEY")
            .expect("OPEN2API_BACKEND_API_KEY must be set");

        let model = env::var("OPEN2API_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-6".to_string());

        // Load authentication keys for accessing the proxy
        // If set, requests must include a valid Bearer token
        let auth_keys = env::var("OPEN2API_API_KEY")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|k| k.trim().to_string())
                    .filter(|k| !k.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Ok(AppConfig {
            host,
            port,
            base_url,
            api_key,
            model,
            auth_keys,
        })
    }
}