use open2api::{
    backend::BackendClient,
    config::AppConfig,
    server::{AppState, create_router},
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    dotenvy::dotenv().ok();

    let config = AppConfig::from_env()?;
    tracing::info!("Loaded Coding Agent backend configuration:");
    tracing::info!("  Base URL: {}", config.base_url);
    tracing::info!("  Model: {}", config.model);

    let client = BackendClient::new(config.clone());
    let state = AppState::new(client, config.clone());
    let app = create_router(state);
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.port);

    tracing::info!("Starting server on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

