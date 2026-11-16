use super::{
    embedding::QueryEmbeddings,
    fusion,
    model::{HybridResponse, HybridSearch},
    service::DocumentSearchService,
};
use crate::{
    app::config::SearchConfig,
    domains::documents::store::chunk::{
        legacy_cosine_similarity, FileChunkResult, VectorFileChunkHit,
    },
    shared::error::{ServiceError, ServiceResult},
};
use futures_util::{future::BoxFuture, stream, FutureExt, StreamExt, TryStreamExt};
use std::{collections::HashSet, future::Future, sync::Arc};
use tokio::{
    sync::Semaphore,
    time::{timeout, Instant},
};

/// Narrow orchestration port also used by deterministic concurrency/race tests.
pub(super) trait HybridBackend: Send + Sync {
    fn deals<'a>(&'a self, actor: i64, deal: &'a str) -> BoxFuture<'a, ServiceResult<Vec<String>>>;
    fn branches<'a>(
        &'a self,
        actor: i64,
        deal: &'a str,
        query: &'a str,
        vector: Arc<Vec<f32>>,
        limit: usize,
    ) -> BoxFuture<'a, ServiceResult<(Vec<FileChunkResult>, Vec<VectorFileChunkHit>)>>;
    fn final_ids(
        &self,
        actor: i64,
        ids: Vec<String>,
    ) -> BoxFuture<'_, ServiceResult<HashSet<String>>>;
}

impl HybridBackend for DocumentSearchService {
    fn deals<'a>(&'a self, actor: i64, deal: &'a str) -> BoxFuture<'a, ServiceResult<Vec<String>>> {
        self.deals(actor, deal).boxed()
    }
    fn branches<'a>(
        &'a self,
        actor: i64,
        deal: &'a str,
        query: &'a str,
        vector: Arc<Vec<f32>>,
        limit: usize,
    ) -> BoxFuture<'a, ServiceResult<(Vec<FileChunkResult>, Vec<VectorFileChunkHit>)>> {
        async move {
            let (keyword, vector) = self
                .read(self.repository.branches(
                    actor,
                    deal,
                    Some(query),
                    Some(vector.as_ref().clone()),
                    limit,
                ))
                .await?;
            Ok((keyword.into_iter().map(|hit| hit.chunk).collect(), vector))
        }
        .boxed()
    }
    fn final_ids(
        &self,
        actor: i64,
        ids: Vec<String>,
    ) -> BoxFuture<'_, ServiceResult<HashSet<String>>> {
        async move {
            phase(
                "final_access",
                self.access()?.current_result_ids(actor, ids),
            )
            .await
        }
        .boxed()
    }
}

pub(super) struct HybridRuntime {
    pub config: SearchConfig,
    pub embeddings: Option<Arc<QueryEmbeddings>>,
    active: Semaphore,
    admitted: Semaphore,
}
impl HybridRuntime {
    pub fn new(config: SearchConfig, embeddings: Option<Arc<QueryEmbeddings>>) -> Self {
        Self {
            active: Semaphore::new(config.active_requests),
            admitted: Semaphore::new(config.active_requests + config.queued_requests),
            config,
            embeddings,
        }
    }
    pub async fn run(
        &self,
        backend: &dyn HybridBackend,
        actor: i64,
        deal: &str,
        search: HybridSearch,
    ) -> ServiceResult<HybridResponse> {
        let started = Instant::now();
        let _admission = self
            .admitted
            .try_acquire()
            .map_err(|_| ServiceError::unavailable("search is busy; retry shortly"))?;
        let result = timeout(self.config.request_timeout, async {
            let _active = phase("queue_wait", async {
                timeout(self.config.queue_timeout, self.active.acquire())
                    .await
                    .map_err(|_| {
                        ServiceError::unavailable("search queue timed out; retry shortly")
                    })?
                    .map_err(|_| ServiceError::unavailable("search unavailable"))
            })
            .await?;
            let deals = phase("scope", backend.deals(actor, deal)).await?;
            if !self.config.enabled {
                return Err(ServiceError::unavailable(
                    "hybrid search is disabled; use keyword search",
                ));
            }
            if deals.is_empty() {
                return Ok(HybridResponse {
                    results: Vec::new(),
                });
            }
            let embeddings = self.embeddings.as_ref().ok_or_else(|| {
                ServiceError::unavailable("semantic search unavailable; use keyword search")
            })?;
            retrieve(backend, embeddings, &self.config, actor, deals, search)
                .await
                .map_err(|error| match error {
                    ServiceError::Internal(_) => ServiceError::unavailable(
                        "hybrid search unavailable; retry or use keyword search",
                    ),
                    other => other,
                })
        })
        .await
        .unwrap_or_else(|_| {
            Err(ServiceError::unavailable(
                "hybrid search timed out; retry or use keyword search",
            ))
        });
        tracing::info!(
            phase = "hybrid_total",
            success = result.is_ok(),
            active_requests = self.config.active_requests - self.active.available_permits(),
            admitted_requests = self.config.active_requests + self.config.queued_requests
                - self.admitted.available_permits(),
            elapsed_seconds = started.elapsed().as_secs_f64()
        );
        result
    }
}

async fn retrieve(
    backend: &dyn HybridBackend,
    embeddings: &QueryEmbeddings,
    config: &SearchConfig,
    actor: i64,
    deals: Vec<String>,
    search: HybridSearch,
) -> ServiceResult<HybridResponse> {
    let budget = search.limit.clamp(50, 100);
    // Provider work completes outside SQL. Both branches then run on one
    // retained repeatable-read transaction per deal, with scope before LIMIT.
    let vector = embeddings.embed(&search.query_text).await?;
    let batches = stream::iter(deals)
        .map(|deal| {
            let vector = vector.clone();
            let query = &search.query_text;
            async move {
                let (mut keyword, semantic) = backend
                    .branches(actor, &deal, query, vector, budget)
                    .await?;
                let mut semantic: Vec<_> = semantic
                    .into_iter()
                    .filter(|hit| {
                        relevant_semantic_distance(hit.distance, config.min_semantic_similarity)
                    })
                    .map(|hit| hit.chunk)
                    .collect();
                keyword.truncate(budget);
                semantic.truncate(budget);
                Ok::<_, ServiceError>(fusion::fuse(keyword, semantic))
            }
        })
        .buffer_unordered(config.deal_concurrency)
        .try_collect::<Vec<_>>()
        .await?;
    let mut results = phase("fusion", async {
        Ok(fusion::top(
            batches.into_iter().flatten().collect(),
            search.limit,
        ))
    })
    .await?;
    let authorized = backend
        .final_ids(actor, results.iter().map(|h| h.chunk_id.clone()).collect())
        .await?;
    results.retain(|h| authorized.contains(&h.chunk_id));
    tracing::info!(result_count = results.len(), "hybrid search completed");
    Ok(HybridResponse { results })
}

fn relevant_semantic_distance(distance: f64, min_similarity: f64) -> bool {
    legacy_cosine_similarity(distance).is_some_and(|similarity| similarity >= min_similarity)
}

pub(super) async fn phase<T>(
    name: &'static str,
    work: impl Future<Output = ServiceResult<T>>,
) -> ServiceResult<T> {
    let start = Instant::now();
    let result = work.await;
    tracing::info!(
        phase = name,
        success = result.is_ok(),
        error_category = match result.as_ref().err() {
            None => "none",
            Some(ServiceError::Forbidden) => "forbidden",
            Some(ServiceError::Unauthorized) => "unauthorized",
            Some(ServiceError::RateLimited) => "rate_limited",
            Some(ServiceError::Validation(_)) => "validation",
            Some(ServiceError::NotFound(_)) => "not_found",
            Some(ServiceError::Conflict(_)) => "conflict",
            Some(ServiceError::Unavailable(_)) => "unavailable",
            Some(ServiceError::Internal(_)) => "internal",
        },
        elapsed_seconds = start.elapsed().as_secs_f64()
    );
    result
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/search/hybrid_tests.rs"]
mod tests;
