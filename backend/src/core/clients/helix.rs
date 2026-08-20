use std::time::{Duration, Instant};

use helix_db::{
    dsl::prelude::{DynamicQueryRequest, DynamicQueryRequestType},
    Client, HelixError,
};
use serde::de::DeserializeOwned;
use tokio::{sync::Mutex, time::sleep};

const DEFAULT_HELIX_URL: &str = "http://127.0.0.1:6969";
const MAX_WRITE_ATTEMPTS: usize = 5;
const INITIAL_WRITE_RETRY_DELAY: Duration = Duration::from_millis(25);

pub struct HelixClient {
    client: Client,
    write_lock: Mutex<()>,
}

impl HelixClient {
    pub fn new() -> Result<Self, String> {
        let url = std::env::var("HELIX_URL").unwrap_or_else(|_| DEFAULT_HELIX_URL.to_string());
        let api_key = std::env::var("HELIX_API_KEY").ok();
        Self::with_config(&url, api_key.as_deref())
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

    pub async fn execute_dynamic_query<R, F>(&self, build_query: F) -> Result<R, String>
    where
        R: DeserializeOwned,
        F: FnOnce() -> DynamicQueryRequest,
    {
        self.execute_dynamic_query_with_context("helix.dynamic_query", "-", 0, build_query)
            .await
    }

    pub async fn execute_document_query<R, F>(
        &self,
        api: &str,
        filename: &str,
        file_size_bytes: u64,
        build_query: F,
    ) -> Result<R, String>
    where
        R: DeserializeOwned,
        F: FnOnce() -> DynamicQueryRequest,
    {
        self.execute_dynamic_query_with_context(api, filename, file_size_bytes, build_query)
            .await
    }

    async fn execute_dynamic_query_with_context<R, F>(
        &self,
        api: &str,
        filename: &str,
        file_size_bytes: u64,
        build_query: F,
    ) -> Result<R, String>
    where
        R: DeserializeOwned,
        F: FnOnce() -> DynamicQueryRequest,
    {
        let query = build_query();
        let is_write = query.request_type == DynamicQueryRequestType::Write;
        let _write_guard = if is_write {
            Some(self.write_lock.lock().await)
        } else {
            None
        };
        let max_attempts = if is_write { MAX_WRITE_ATTEMPTS } else { 1 };

        for attempt in 1..=max_attempts {
            let started_at = Instant::now();

            match self.client.query().dynamic(query.clone()).send().await {
                Ok(response) => {
                    tracing::info!(
                        api,
                        filename,
                        file_size_bytes,
                        elapsed_seconds = started_at.elapsed().as_secs_f64(),
                    );
                    return Ok(response);
                }
                Err(error) if attempt < max_attempts && is_concurrent_write_conflict(&error) => {
                    let delay = write_retry_delay(attempt);
                    tracing::warn!(
                        api,
                        filename,
                        file_size_bytes,
                        reason = %error,
                        attempt,
                        elapsed_seconds = started_at.elapsed().as_secs_f64(),
                    );
                    sleep(delay).await;
                }
                Err(error) => {
                    tracing::error!(
                        api,
                        filename,
                        file_size_bytes,
                        reason = %error,
                        elapsed_seconds = started_at.elapsed().as_secs_f64(),
                    );
                    return Err(format!("failed to execute Helix query: {error}"));
                }
            }
        }

        unreachable!("Helix query attempt loop always returns")
    }
}

fn is_concurrent_write_conflict(error: &HelixError) -> bool {
    matches!(
        error,
        HelixError::RemoteError { details }
            if details.contains("request conflicted with a concurrent write")
    )
}

fn write_retry_delay(attempt: usize) -> Duration {
    INITIAL_WRITE_RETRY_DELAY.saturating_mul(1_u32 << (attempt - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_concurrent_write_conflicts_as_retryable() {
        let conflict = HelixError::RemoteError {
            details: r#"{"error":"request conflicted with a concurrent write; please retry"}"#
                .to_string(),
        };
        let other_error = HelixError::RemoteError {
            details: r#"{"error":"unique constraint violation"}"#.to_string(),
        };

        assert!(is_concurrent_write_conflict(&conflict));
        assert!(!is_concurrent_write_conflict(&other_error));
    }

    #[test]
    fn write_retry_delay_uses_bounded_exponential_backoff() {
        assert_eq!(write_retry_delay(1), Duration::from_millis(25));
        assert_eq!(write_retry_delay(2), Duration::from_millis(50));
        assert_eq!(write_retry_delay(4), Duration::from_millis(200));
    }
}
