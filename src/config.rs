use anyhow::Result;
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub default_max_tokens: u32,
    pub auth_keys: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let host = env::var("OPEN2API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let port = env::var("OPEN2API_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let base_url = env::var("OPEN2API_BACKEND_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

        let api_key =
            env::var("OPEN2API_BACKEND_API_KEY").expect("OPEN2API_BACKEND_API_KEY must be set");
        let model = env::var("OPEN2API_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string());

        let auth_keys = env::var("OPEN2API_API_KEY")
            .ok()
            .map(|s| {
                s.split(',')
                    .map(|k| k.trim().to_string())
                    .filter(|k| !k.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let default_max_tokens = env::var("OPEN2API_DEFAULT_MAX_TOKENS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(128 * 1024);

        Ok(AppConfig {
            host,
            port,
            base_url,
            api_key,
            model,
            default_max_tokens,
            auth_keys,
        })
    }
}

