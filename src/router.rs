use crate::config::BackendConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Client identifier for routing
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ClientId {
    ip_hash: [u8; 32],
    ua_hash: [u8; 32],
}

impl ClientId {
    /// Create a client ID from IP and User-Agent
    pub fn new(ip: &str, user_agent: &str) -> Self {
        let ip_hash = blake3::hash(ip.as_bytes());
        let ua_hash = blake3::hash(user_agent.as_bytes());

        ClientId {
            ip_hash: *ip_hash.as_bytes(),
            ua_hash: *ua_hash.as_bytes(),
        }
    }

    /// Get combined hash for routing
    pub fn routing_hash(&self) -> u64 {
        // Combine both hashes to create a routing key
        let combined: Vec<u8> = self.ip_hash.iter().chain(self.ua_hash.iter()).copied().collect();
        let hash = blake3::hash(&combined);
        u64::from_be_bytes(hash.as_bytes()[..8].try_into().unwrap())
    }
}

/// Backend health status
#[derive(Debug, Clone)]
pub struct BackendHealth {
    pub name: String,
    pub is_healthy: bool,
    pub last_check: std::time::Instant,
    pub failure_count: usize,
}

/// Backend pool with health tracking
#[derive(Clone)]
pub struct BackendPool {
    backends: Arc<Vec<BackendConfig>>,
    health_status: Arc<RwLock<HashMap<String, BackendHealth>>>,
}

impl BackendPool {
    /// Create a new backend pool
    pub fn new(backends: Vec<BackendConfig>) -> Self {
        let health_status: HashMap<String, BackendHealth> = backends
            .iter()
            .map(|b| {
                (
                    b.name.clone(),
                    BackendHealth {
                        name: b.name.clone(),
                        is_healthy: true,
                        last_check: std::time::Instant::now(),
                        failure_count: 0,
                    },
                )
            })
            .collect();

        BackendPool {
            backends: Arc::new(backends),
            health_status: Arc::new(RwLock::new(health_status)),
        }
    }

    /// Get all backends
    pub fn get_all(&self) -> &[BackendConfig] {
        &self.backends
    }

    /// Get backend by index
    pub fn get_by_index(&self, index: usize) -> Option<&BackendConfig> {
        self.backends.get(index)
    }

    /// Get backend by name
    pub fn get_by_name(&self, name: &str) -> Option<&BackendConfig> {
        self.backends.iter().find(|b| b.name == name)
    }

    /// Get index of backend by name
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.backends.iter().position(|b| b.name == name)
    }

    /// Mark a backend as failed
    pub async fn mark_failed(&self, name: &str) {
        let mut health = self.health_status.write().await;
        if let Some(h) = health.get_mut(name) {
            h.failure_count += 1;
            h.last_check = std::time::Instant::now();

            // Mark as unhealthy after 3 consecutive failures
            if h.failure_count >= 3 {
                h.is_healthy = false;
                tracing::warn!("Backend {} marked as unhealthy after {} failures", name, h.failure_count);
            }
        }
    }

    /// Mark a backend as healthy
    pub async fn mark_healthy(&self, name: &str) {
        let mut health = self.health_status.write().await;
        if let Some(h) = health.get_mut(name) {
            h.is_healthy = true;
            h.failure_count = 0;
            h.last_check = std::time::Instant::now();
        }
    }

    /// Check if a backend is healthy
    pub async fn is_healthy(&self, name: &str) -> bool {
        let health = self.health_status.read().await;
        health.get(name).map(|h| h.is_healthy).unwrap_or(false)
    }

    /// Get health status summary
    pub async fn get_health_summary(&self) -> HashMap<String, bool> {
        let health = self.health_status.read().await;
        health.iter().map(|(k, v)| (k.clone(), v.is_healthy)).collect()
    }
}

/// Router for client request routing
#[derive(Clone)]
pub struct Router {
    pool: BackendPool,
    /// Consistent mapping of client IDs to backend indices
    client_backend_map: Arc<RwLock<HashMap<u64, usize>>>,
}

impl Router {
    /// Create a new router
    pub fn new(pool: BackendPool) -> Self {
        Router {
            pool,
            client_backend_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create router from backend configs
    pub fn from_backends(backends: Vec<BackendConfig>) -> Self {
        let pool = BackendPool::new(backends);
        Router::new(pool)
    }

    /// Route a client to a backend using consistent hashing
    pub async fn route(&self, client_id: &ClientId) -> BackendConfig {
        let hash = client_id.routing_hash();
        let backends = self.pool.get_all();

        if backends.is_empty() {
            panic!("No backends available");
        }

        // Check if we have a cached mapping and backend is healthy
        {
            let map = self.client_backend_map.read().await;
            if let Some(index) = map.get(&hash) {
                if let Some(backend) = self.pool.get_by_index(*index) {
                    if self.pool.is_healthy(&backend.name).await {
                        return backend.clone();
                    }
                }
            }
        }

        // Find a healthy backend using consistent hashing
        let backend = self.find_backend_by_hash(hash).await;

        // Cache the mapping
        {
            let mut map = self.client_backend_map.write().await;
            let index = self.pool.index_of(&backend.name).unwrap_or(0);
            map.insert(hash, index);
        }

        backend
    }

    /// Find backend using consistent hash
    async fn find_backend_by_hash(&self, hash: u64) -> BackendConfig {
        let backends = self.pool.get_all();
        if backends.is_empty() {
            panic!("No backends available");
        }

        // Use modulo for simple consistent distribution
        let base_index = (hash % backends.len() as u64) as usize;

        // Try to use the hashed backend first
        if self.pool.is_healthy(&backends[base_index].name).await {
            return backends[base_index].clone();
        }

        // Fallback: find next healthy backend
        for i in 0..backends.len() {
            let index = (base_index + i) % backends.len();
            if self.pool.is_healthy(&backends[index].name).await {
                return backends[index].clone();
            }
        }

        // All unhealthy, return the hashed one anyway
        backends[base_index].clone()
    }

    /// Get the backend pool
    pub fn pool(&self) -> &BackendPool {
        &self.pool
    }

    /// Report a backend failure
    pub async fn report_failure(&self, backend_name: &str) {
        self.pool.mark_failed(backend_name).await;

        // Clear cached mappings for failed backend
        let mut map = self.client_backend_map.write().await;
        let index = self.pool.index_of(backend_name);
        if let Some(idx) = index {
            map.retain(|_, &mut v| v != idx);
        }
    }

    /// Report a backend success
    pub async fn report_success(&self, backend_name: &str) {
        self.pool.mark_healthy(backend_name).await;
    }
}