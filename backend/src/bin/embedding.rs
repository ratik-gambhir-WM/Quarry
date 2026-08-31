use anyhow::{Error, Result};
use quarry_backend::{config::AppConfig, core::clients::openai::OpenAiClient};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("quarry_backend=info")
        .try_init();
    let config = AppConfig::from_env().map_err(Error::msg)?;
    let openai = config
        .openai
        .as_ref()
        .ok_or_else(|| Error::msg("OpenAI capability is not configured"))?;

    let chunks = vec![
        "The architecture is designed around a modular service model where each service owns a clearly defined business capability. This approach improves maintainability by reducing coupling between teams and application components. Services communicate through well-defined APIs for synchronous workflows and publish events for asynchronous updates, allowing the platform to scale and evolve without requiring tightly coordinated releases.",

        "Data management follows a service-owned persistence pattern, where each domain service is responsible for its own schema, storage technology, and access rules. Customer profile data is stored in a relational database to support consistency and transactional integrity, while interaction history is stored in a document database to accommodate flexible event payloads. Aggregated data is periodically streamed into an analytics warehouse for reporting and business intelligence.",

        "Security controls are applied across identity, network, application, and data layers. Users authenticate through an enterprise identity provider, while APIs are protected using token-based authorization and role-based access policies. Sensitive information is encrypted in transit and at rest, secrets are stored in a managed vault, and audit logs are retained for operational monitoring, compliance reviews, and incident investigation.",
    ];
    let client = OpenAiClient::from_config(reqwest::Client::new(), openai);
    let embeddings = client
        .gen_embeddings(&chunks, Some("text-embedding-3-small"))
        .await
        .map_err(Error::msg)?;

    for (index, embedding) in embeddings.into_iter().enumerate() {
        println!("Embedding {} has {} dimensions", index, embedding.len());
        println!("Embedding {index} values: {embedding:?}");
    }

    Ok(())
}
