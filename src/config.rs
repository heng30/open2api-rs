use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;

/// Configuration for a single backend
#[derive(Debug, Clone)]
pub struct BackendConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub backends: Vec<BackendConfig>,
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let backends = Self::discover_backends()?;

        if backends.is_empty() {
            anyhow::bail!("No backends configured. Set at least BACKEND_1_BASE_URL and BACKEND_1_API_KEY");
        }

        Ok(AppConfig {
            host,
            port,
            backends,
        })
    }

    /// Discover backends from environment variables
    /// Looks for patterns like BACKEND_1_BASE_URL, BACKEND_1_API_KEY, BACKEND_1_NAME
    fn discover_backends() -> Result<Vec<BackendConfig>> {
        let mut backends: Vec<BackendConfig> = Vec::new();
        let mut backend_map: HashMap<String, HashMap<String, String>> = HashMap::new();

        // Collect all BACKEND_* environment variables
        for (key, value) in env::vars() {
            if !key.starts_with("BACKEND_") {
                continue;
            }

            // Parse key: BACKEND_1_BASE_URL -> ("1", "BASE_URL", value)
            let rest = key.strip_prefix("BACKEND_").unwrap();
            if let Some(underscore_pos) = rest.find('_') {
                let id = &rest[..underscore_pos];
                let field = &rest[underscore_pos + 1..];

                backend_map
                    .entry(id.to_string())
                    .or_default()
                    .insert(field.to_string(), value);
            }
        }

        // Build BackendConfig from collected values
        let mut ids: Vec<String> = backend_map.keys().cloned().collect();
        ids.sort();

        for id in ids {
            let fields = backend_map.get(&id).context(format!("Backend {} fields", id))?;

            let base_url = fields
                .get("BASE_URL")
                .context(format!("BACKEND_{}_BASE_URL not set", id))?
                .clone();

            let api_key = fields
                .get("API_KEY")
                .context(format!("BACKEND_{}_API_KEY not set", id))?
                .clone();

            let name = fields
                .get("NAME")
                .cloned()
                .unwrap_or_else(|| format!("backend-{}", id));

            backends.push(BackendConfig {
                name,
                base_url,
                api_key,
            });
        }

        Ok(backends)
    }
}