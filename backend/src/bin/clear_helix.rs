use anyhow::{Context, Result};
use helix_db::dsl::prelude::*;
use quarry_backend::core::clients::helix::HelixClient;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter("quarry_backend=info")
        .try_init();

    let helix = HelixClient::new().map_err(anyhow::Error::msg)?;
    let before: Value = helix
        .execute_dynamic_query(count_all_nodes)
        .await
        .map_err(anyhow::Error::msg)?;
    println!("Helix nodes before cleanup: {before}");

    let deleted: Value = helix
        .execute_dynamic_query(delete_all_nodes)
        .await
        .map_err(anyhow::Error::msg)?;
    println!("Helix cleanup response: {deleted}");

    let after: Value = helix
        .execute_dynamic_query(count_all_nodes)
        .await
        .map_err(anyhow::Error::msg)?;
    println!("Helix nodes after cleanup: {after}");
    verify_zero_nodes(&after).context("Helix cleanup verification failed")?;
    Ok(())
}

fn count_all_nodes() -> DynamicQueryRequest {
    DynamicQueryRequest::read(
        read_batch()
            .var_as("node_count", g().n(NodeRef::all()).count())
            .returning(["node_count"]),
    )
}

fn delete_all_nodes() -> DynamicQueryRequest {
    DynamicQueryRequest::write(
        write_batch()
            .var_as("deleted_nodes", g().n(NodeRef::all()).drop().count())
            .returning(["deleted_nodes"]),
    )
}

fn verify_zero_nodes(response: &Value) -> Result<()> {
    let count = response
        .pointer("/node_count/count")
        .or_else(|| response.pointer("/nodeCount/count"))
        .or_else(|| response.pointer("/node_count/value"))
        .or_else(|| response.pointer("/nodeCount/value"))
        .or_else(|| response.pointer("/node_count"))
        .or_else(|| response.pointer("/nodeCount"));
    match count.and_then(Value::as_u64) {
        Some(0) => Ok(()),
        Some(count) => anyhow::bail!("{count} Helix nodes remain"),
        None => anyhow::bail!("unexpected count response: {response}"),
    }
}
