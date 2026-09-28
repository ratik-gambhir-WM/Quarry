use std::time::Instant;

use helix_db::{Client, HelixError, QueryRequest, QueryRequestType};
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::app::config::HelixConfig;

pub struct HelixClient {
    client: Client,
    write_lock: Mutex<()>,
}

impl HelixClient {
    pub fn from_config(config: &HelixConfig) -> Result<Self, String> {
        Self::with_config(
            &config.url,
            config.api_key.as_ref().map(|api_key| api_key.expose()),
        )
    }

    pub fn with_config(url: &str, api_key: Option<&str>) -> Result<Self, String> {
        let client = Client::new(Some(url))
            .map_err(|err| format!("failed to create Helix client for `{url}`: {err}"))?
            .with_api_key(api_key);
        Ok(Self {
            client,
            write_lock: Mutex::new(()),
        })
    }

    pub async fn execute_read_query<R>(&self, api: &str, query: QueryRequest) -> Result<R, String>
    where
        R: DeserializeOwned,
    {
        self.execute_query(api, query, QueryIntent::Read).await
    }

    pub async fn execute_write_query<R>(&self, api: &str, query: QueryRequest) -> Result<R, String>
    where
        R: DeserializeOwned,
    {
        self.execute_query(api, query, QueryIntent::Write).await
    }

    async fn execute_query<R>(
        &self,
        api: &str,
        query: QueryRequest,
        intent: QueryIntent,
    ) -> Result<R, String>
    where
        R: DeserializeOwned,
    {
        if query.request_type() != intent.request_type() {
            return Err(format!(
                "Helix query intent mismatch: expected {} request",
                intent.name()
            ));
        }
        let started_at = Instant::now();
        let result = match intent {
            QueryIntent::Read => self.client.query(query).send().await,
            QueryIntent::Write => {
                let _write_guard = self.write_lock.lock().await;
                self.client
                    .request_builder()
                    .writer_only()
                    .should_await_durability(true)
                    .query(query)
                    .send()
                    .await
            }
        };

        match result {
            Ok(response) => {
                tracing::info!(
                    api,
                    operation = intent.name(),
                    elapsed_seconds = started_at.elapsed().as_secs_f64(),
                );
                Ok(response)
            }
            Err(error) => {
                // v3 intentionally exposes a non-200 response only as text. It does not retain
                // an HTTP status, protocol code, or retryability flag, so retrying a write could
                // replay an already accepted request. Treat every failed write as terminal.
                tracing::error!(
                    api,
                    operation = intent.name(),
                    error_kind = helix_error_kind(&error),
                    elapsed_seconds = started_at.elapsed().as_secs_f64(),
                );
                Err("failed to execute Helix query".to_string())
            }
        }
    }
}

#[derive(Clone, Copy)]
enum QueryIntent {
    Read,
    Write,
}

impl QueryIntent {
    const fn request_type(self) -> QueryRequestType {
        match self {
            Self::Read => QueryRequestType::Read,
            Self::Write => QueryRequestType::Write,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

const fn helix_error_kind(error: &HelixError) -> &'static str {
    match error {
        HelixError::ReqwestError(_) => "transport",
        HelixError::RemoteError { .. } => "remote",
        HelixError::SerializationError(_) => "serialization",
        HelixError::InvalidURL(_) => "invalid_url",
        HelixError::InvalidRequest { .. } => "invalid_request",
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/adapters/helix/client_tests.rs"]
mod tests;
