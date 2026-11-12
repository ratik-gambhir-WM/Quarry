//! Read-only readiness check for the configured PostgreSQL database.
use quarry_backend::{adapters::postgres::client::PostgresClient, AppConfig};
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let config = AppConfig::from_env().map_err(anyhow::Error::msg)?;
    let client = PostgresClient::connect(&config.postgres).await?;
    client.close().await;
    println!("Quarry PostgreSQL connection is ready");
    Ok(())
}
