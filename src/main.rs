use open2api::config::AppConfig;
use open2api::router::Router;
use open2api::backend::BackendClient;
use open2api::server::{create_router, AppState};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Load configuration
    let config = AppConfig::from_env()?;
    tracing::info!("Loaded {} backend(s)", config.backends.len());
    for backend in &config.backends {
        tracing::info!(
            "  Backend: {} -> {}",
            backend.name,
            backend.base_url
        );
    }

    // Create router and backend client
    let router = Router::from_backends(config.backends.clone());
    let client = BackendClient::new(router);

    // Create app state
    let state = AppState::new(client, config.clone());

    // Create Axum router
    let app = create_router(state);

    // Bind to address
    let addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        config.port,
    );

    tracing::info!("Starting server on {}", addr);

    let listener = TcpListener::bind(addr).await?;

    // Run server - axum 0.8 uses into_make_service_with_connect_info for ConnectInfo extractor
    axum::serve(listener, app.into_make_service())
        .await?;

    Ok(())
}