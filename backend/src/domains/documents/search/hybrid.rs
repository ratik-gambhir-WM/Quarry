use super::{
    embedding::QueryEmbeddingCache,
    fusion::{self, Identity},
    model::{HybridResponse, HybridSearch},
    service::DocumentSearchService,
};
use crate::{
    app::config::SearchConfig,
    domains::{
        documents::graph::query::{
            helix_cosine_similarity, FileChunkKeywordSearch, FileChunkResult,
            FileChunkVectorSearch, VectorFileChunkHit,
        },
        identity::access::CanonicalChunk,
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
    fn prepare<'a>(
        &'a self,
        actor: i64,
        deal: &'a str,
    ) -> BoxFuture<'a, ServiceResult<Vec<String>>>;
    fn keyword<'a>(
        &'a self,
        deal: &'a str,
        ids: Vec<String>,
        query: &'a str,
        limit: usize,
    ) -> BoxFuture<'a, ServiceResult<Vec<FileChunkResult>>>;
    fn vector<'a>(
        &'a self,
        deal: &'a str,
        ids: Vec<String>,
        vector: Arc<Vec<f32>>,
        limit: usize,
    ) -> BoxFuture<'a, ServiceResult<Vec<VectorFileChunkHit>>>;
    fn fresh<'a>(
        &'a self,
        actor: i64,
        deal: &'a str,
    ) -> BoxFuture<'a, ServiceResult<HashSet<Identity>>>;
    fn final_ids(
        &self,
        actor: i64,
        ids: Vec<String>,
    ) -> BoxFuture<'_, ServiceResult<HashSet<String>>>;
}

pub(super) fn canonical_identities(deal: &str, chunks: Vec<CanonicalChunk>) -> HashSet<Identity> {
    chunks
        .into_iter()
        .map(|c| {
            (
                deal.to_owned(),
                c.file_id,
                c.version_id,
                c.identity.chunk_id,
                c.identity.index_generation,
                c.identity.chunk_sha256,
                c.identity.chunk_index,
            )
        })
        .collect()
}

impl HybridBackend for DocumentSearchService {
    fn deals<'a>(&'a self, actor: i64, deal: &'a str) -> BoxFuture<'a, ServiceResult<Vec<String>>> {
        self.deals(actor, deal).boxed()
    }
    fn prepare<'a>(
        &'a self,
        actor: i64,
        deal: &'a str,
    ) -> BoxFuture<'a, ServiceResult<Vec<String>>> {
        async move {
            let candidates = phase(
                "candidate_snapshot",
                self.access()?.current_chunks(actor, deal),
            )
            .await?;
            phase(
                "header_verification",
                self.read(self.graph.verify_candidates(
                    deal.to_owned(),
                    candidates.iter().map(|c| c.identity.clone()).collect(),
                )),
            )
            .await?;
            Ok(candidates
                .into_iter()
                .map(|c| c.identity.chunk_id)
                .collect())
        }
        .boxed()
    }
    fn keyword<'a>(
        &'a self,
        deal: &'a str,
        ids: Vec<String>,
        query: &'a str,
        limit: usize,
    ) -> BoxFuture<'a, ServiceResult<Vec<FileChunkResult>>> {
        async move {
            let hits = phase(
                "keyword",
                self.read(self.graph.keyword_current(
                    FileChunkKeywordSearch {
                        deal_id: deal.into(),
                        query_text: query.into(),
                        limit,
                    },
                    ids,
                )),
            )
            .await?;
            Ok(hits.into_iter().map(|h| h.chunk).collect())
        }
        .boxed()
    }
    fn vector<'a>(
        &'a self,
        deal: &'a str,
        ids: Vec<String>,
        vector: Arc<Vec<f32>>,
        limit: usize,
    ) -> BoxFuture<'a, ServiceResult<Vec<VectorFileChunkHit>>> {
        async move {
            let hits = phase(
                "vector",
                self.read(self.graph.vector_current(
                    FileChunkVectorSearch {
                        deal_id: deal.into(),
                        query_embedding: vector.as_ref().clone(),
                        limit,
                    },
                    ids,
                )),
            )
            .await?;
            Ok(hits)
        }
        .boxed()
    }
    fn fresh<'a>(
        &'a self,
        actor: i64,
        deal: &'a str,
    ) -> BoxFuture<'a, ServiceResult<HashSet<Identity>>> {
        async move {
            Ok(canonical_identities(
                deal,
                phase("freshness", self.access()?.current_chunks(actor, deal)).await?,
            ))
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
    pub embeddings: Option<Arc<QueryEmbeddingCache>>,
    active: Semaphore,
    admitted: Semaphore,
}
impl HybridRuntime {
    pub fn new(config: SearchConfig, embeddings: Option<Arc<QueryEmbeddingCache>>) -> Self {
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
    embeddings: &QueryEmbeddingCache,
    config: &SearchConfig,
    actor: i64,
    deals: Vec<String>,
    search: HybridSearch,
) -> ServiceResult<HybridResponse> {
    let budget = search.limit.clamp(50, 100);
    // Shared request-owned future: keyword never waits for embedding; each vector branch polls
    // the same provider/cache outcome. First error drops the stream and every remaining waiter.
    let embedding = embeddings
        .embed(&search.query_text)
        .map(|r| r.map_err(Arc::new))
        .shared();
    let pipelines = async {
        stream::iter(deals)
            .map(|deal| {
                let embedding = embedding.clone();
                let query = &search.query_text;
                async move {
                    let ids = backend.prepare(actor, &deal).await?;
                    let keyword = backend.keyword(&deal, ids.clone(), query, budget);
                    let semantic = async {
                        let vector = embedding
                            .await
                            .map_err(|e| ServiceError::unavailable(e.to_string()))?;
                        backend.vector(&deal, ids, vector, budget).await
                    };
                    let (mut keyword, semantic) = tokio::try_join!(keyword, semantic)?;
                    // Nearest neighbors are candidates, not proof of relevance. Filter before
                    // RRF discards distances; weak neighbors must not fill an otherwise empty list.
                    let mut semantic: Vec<_> = semantic
                        .into_iter()
                        .filter(|hit| {
                            relevant_semantic_distance(hit.distance, config.min_semantic_similarity)
                        })
                        .map(|hit| hit.chunk)
                        .collect();
                    let current = backend.fresh(actor, &deal).await?;
                    keyword.retain(|c| current.contains(&fusion::identity(c)));
                    semantic.retain(|c| current.contains(&fusion::identity(c)));
                    // Enforce branch budgets even if a collaborator returns excess candidates.
                    keyword.truncate(budget);
                    semantic.truncate(budget);
                    Ok::<_, ServiceError>(fusion::fuse(keyword, semantic))
                }
            })
            .buffer_unordered(config.deal_concurrency)
            .try_collect::<Vec<_>>()
            .await
    };
    let shared_embedding = embedding.clone();
    let drive_embedding = async {
        shared_embedding
            .await
            .map_err(|e| ServiceError::unavailable(e.to_string()))
    };
    let (_, batches) = tokio::try_join!(drive_embedding, pipelines)?;
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
    helix_cosine_similarity(distance).is_some_and(|similarity| similarity >= min_similarity)
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
