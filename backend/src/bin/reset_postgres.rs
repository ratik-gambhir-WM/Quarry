//! Explicit, operator-invoked MVP schema reset. Never part of startup or tests.
use anyhow::{bail, Result};
use quarry_backend::{
    adapters::postgres::client::PostgresClient, app::migrations::reset_product_schema, AppConfig,
};
#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--confirm-reset"] {
        bail!("usage: reset_postgres --confirm-reset (permanently recreates the selected Quarry product schema)");
    }
    let _ = dotenvy::dotenv();
    let config = AppConfig::from_env().map_err(anyhow::Error::msg)?;
    let client = PostgresClient::connect(&config.postgres).await?;
    reset_product_schema(&client, config.helix.vector_dimension.get()).await?;
    client.close().await;
    println!("Recreated Quarry PostgreSQL schema 3");
    Ok(())
}
