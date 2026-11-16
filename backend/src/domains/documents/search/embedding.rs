//! Request-owned query embeddings without caching or request coalescing.
use crate::{
    adapters::openai::client::OpenAiClient,
    shared::error::{ServiceError, ServiceResult},
};
use futures_util::future::BoxFuture;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::Semaphore,
    time::{timeout, Instant},
};

pub(crate) trait EmbeddingProvider: Send + Sync {
    fn embed<'a>(&'a self, query: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>>;
}

pub(super) struct OpenAiQueryEmbedding {
    pub client: Arc<OpenAiClient>,
    pub model: String,
    pub dimensions: usize,
}
impl EmbeddingProvider for OpenAiQueryEmbedding {
    fn embed<'a>(&'a self, query: &'a str) -> BoxFuture<'a, ServiceResult<Vec<f32>>> {
        Box::pin(async move {
            let vectors = self
                .client
                .gen_private_embeddings(&[query], Some(&self.model))
                .await
                .map_err(|_| unavailable())?;
            validate_vector(vectors, self.dimensions)
        })
    }
}
fn validate_vector(mut vectors: Vec<Vec<f64>>, dimensions: usize) -> ServiceResult<Vec<f32>> {
    if vectors.len() != 1 || dimensions == 0 || vectors[0].len() != dimensions {
        return Err(unavailable());
    }
    let values = vectors.pop().expect("one vector checked");
    crate::adapters::postgres::vector::CosineVector::from_f64(&values, dimensions)
        .map(|vector| vector.as_slice().to_vec())
        .map_err(|_| unavailable())
}
fn unavailable() -> ServiceError {
    ServiceError::unavailable("semantic search unavailable; retry or use keyword search")
}

pub struct QueryEmbeddings {
    provider: Arc<dyn EmbeddingProvider>,
    providers: Semaphore,
    deadline: Duration,
}
impl QueryEmbeddings {
    pub(crate) fn openai(
        client: Arc<OpenAiClient>,
        model: String,
        dimensions: usize,
        concurrency: usize,
        deadline: Duration,
    ) -> Self {
        Self::new(
            Arc::new(OpenAiQueryEmbedding {
                client,
                model,
                dimensions,
            }),
            concurrency,
            deadline,
        )
    }
    pub(crate) fn new(
        provider: Arc<dyn EmbeddingProvider>,
        concurrency: usize,
        deadline: Duration,
    ) -> Self {
        Self {
            provider,
            providers: Semaphore::new(concurrency),
            deadline,
        }
    }
    pub(super) async fn embed(&self, query: &str) -> ServiceResult<Arc<Vec<f32>>> {
        let started = Instant::now();
        let result = timeout(self.deadline, async {
            let _permit = self.providers.acquire().await.map_err(|_| unavailable())?;
            self.provider.embed(query).await.map(Arc::new)
        })
        .await
        .map_err(|_| unavailable())?;
        tracing::info!(
            phase = "embedding",
            success = result.is_ok(),
            elapsed_seconds = started.elapsed().as_secs_f64()
        );
        result
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/search/embedding_tests.rs"]
mod tests;
