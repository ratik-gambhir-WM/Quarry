use anyhow::{Context, Result};
use quarry_backend::{
    config::AppConfig, create_router, repository::document_repository::ensure_document_indexes,
    state::AppState,
};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let config = AppConfig::from_env().map_err(anyhow::Error::msg)?;
    let state = AppState::new().map_err(anyhow::Error::msg)?;
    ensure_document_indexes(state.helix())
        .await
        .map_err(anyhow::Error::msg)
        .context("failed to initialize Helix document indexes")?;
    let app = create_router(state, &config);
    let listener = TcpListener::bind(config.bind_address)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_address))?;

    tracing::info!(address = %config.bind_address, "Quarry API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("API server failed")?;

    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "quarry_backend=info,tower_http=info".into());

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install shutdown signal handler");
        return;
    }
    tracing::info!("shutdown signal received");
}
