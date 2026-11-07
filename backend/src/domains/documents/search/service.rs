use super::{fusion::identity, hybrid::HybridBackend};
use crate::{
    domains::{
        documents::graph::{
            query::{
                FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit,
                VectorFileChunkHit,
            },
            repository::DocumentGraphSearch,
        },
        identity::access::ResourceAccess,
        memberships::model::Capability,
    },
    shared::error::{ServiceError, ServiceResult},
};
use futures_util::{stream, StreamExt, TryStreamExt};
#[derive(Clone)]
pub struct DocumentSearchService {
    pub(super) graph: DocumentGraphSearch,
    reads: std::sync::Arc<tokio::sync::Semaphore>,
    pub(super) hybrid: std::sync::Arc<super::hybrid::HybridRuntime>,
    access: Option<ResourceAccess>,
}
impl DocumentSearchService {
    pub fn with_hybrid(
        mut self,
        config: crate::app::config::SearchConfig,
        embeddings: Option<std::sync::Arc<super::embedding::QueryEmbeddingCache>>,
    ) -> Self {
        self.reads = std::sync::Arc::new(tokio::sync::Semaphore::new(config.helix_reads));
        self.hybrid = std::sync::Arc::new(super::hybrid::HybridRuntime::new(config, embeddings));
        self
    }
    pub async fn hybrid_owned(
        &self,
        search: super::model::HybridSearch,
        actor: i64,
        deal: &str,
    ) -> ServiceResult<super::model::HybridResponse> {
        self.hybrid.run(self, actor, deal, search).await
    }
    pub(super) async fn read<T>(
        &self,
        work: impl std::future::Future<Output = Result<T, crate::shared::error::RepositoryError>>,
    ) -> ServiceResult<T> {
        let _permit = super::hybrid::phase("helix_queue", async {
            self.reads
                .acquire()
                .await
                .map_err(|_| ServiceError::unavailable("search unavailable"))
        })
        .await?;
        super::hybrid::phase("helix_read", async {
            work.await.map_err(ServiceError::from)
        })
        .await
    }

    pub fn new(graph: DocumentGraphSearch) -> Self {
        Self {
            graph,
            access: None,
            reads: std::sync::Arc::new(tokio::sync::Semaphore::new(8)),
            hybrid: std::sync::Arc::new(super::hybrid::HybridRuntime::new(
                Default::default(),
                None,
            )),
        }
    }
    pub fn with_access(mut self, access: ResourceAccess) -> Self {
        self.access = Some(access);
        self
    }
    pub(super) fn access(&self) -> ServiceResult<&ResourceAccess> {
        self.access
            .as_ref()
            .ok_or_else(|| ServiceError::internal("search authorization unavailable"))
    }
    pub(super) async fn deals(&self, actor: i64, deal: &str) -> ServiceResult<Vec<String>> {
        let access = self.access()?;
        if !deal.is_empty() {
            access
                .policy()
                .require(deal, actor, Capability::Read)
                .await?;
            return Ok(vec![deal.into()]);
        }
        let deals = access.policy().accessible_deals(actor).await?;
        if deals.len() > 200 {
            return Err(ServiceError::unavailable(
                "global search exceeds 200 deals; search one deal",
            ));
        }
        Ok(deals)
    }
    pub async fn vector_owned(
        &self,
        search: FileChunkVectorSearch,
        actor: i64,
    ) -> ServiceResult<Vec<VectorFileChunkHit>> {
        let access = self.access()?;
        let batches: Vec<Vec<_>> = stream::iter(self.deals(actor, &search.deal_id).await?)
            .map(|deal| {
                let search = &search;
                async move {
                    let candidates =
                        super::hybrid::HybridBackend::prepare(self, actor, &deal).await?;
                    let hits = self
                        .read(self.graph.vector_current(
                            FileChunkVectorSearch {
                                deal_id: deal.clone(),
                                ..(*search).clone()
                            },
                            candidates,
                        ))
                        .await?;
                    let current = self.fresh(actor, &deal).await?;
                    Ok::<_, ServiceError>(
                        hits.into_iter()
                            .filter(|h| current.contains(&identity(&h.chunk)))
                            .collect(),
                    )
                }
            })
            .buffer_unordered(self.hybrid.config.deal_concurrency)
            .try_collect()
            .await?;
        let mut all: Vec<VectorFileChunkHit> = batches.into_iter().flatten().collect();
        // Cosine distances from the same configured embedding model are directly comparable.
        all.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then(a.chunk.chunk_id.cmp(&b.chunk.chunk_id))
        });
        let mut seen = std::collections::HashSet::new();
        all.retain(|h| seen.insert(h.chunk.chunk_id.clone()));
        all.truncate(search.limit);
        let authorized = access
            .current_result_ids(
                actor,
                all.iter().map(|hit| hit.chunk.chunk_id.clone()).collect(),
            )
            .await?;
        all.retain(|hit| authorized.contains(&hit.chunk.chunk_id));
        Ok(all)
    }
    pub async fn keyword_owned(
        &self,
        search: FileChunkKeywordSearch,
        actor: i64,
    ) -> ServiceResult<Vec<KeywordFileChunkHit>> {
        let access = self.access()?;
        let batches: Vec<Vec<_>> = stream::iter(self.deals(actor, &search.deal_id).await?)
            .map(|deal| {
                let search = &search;
                async move {
                    let candidates =
                        super::hybrid::HybridBackend::prepare(self, actor, &deal).await?;
                    let hits = self
                        .read(self.graph.keyword_current(
                            FileChunkKeywordSearch {
                                deal_id: deal.clone(),
                                ..(*search).clone()
                            },
                            candidates,
                        ))
                        .await?;
                    let current = self.fresh(actor, &deal).await?;
                    let mut retained = Vec::new();
                    for (rank, mut hit) in hits.into_iter().enumerate() {
                        if current.contains(&identity(&hit.chunk)) {
                            hit.score = 1.0 / (60.0 + rank as f64 + 1.0);
                            retained.push(hit);
                        }
                    }
                    Ok::<_, ServiceError>(retained)
                }
            })
            .buffer_unordered(self.hybrid.config.deal_concurrency)
            .try_collect()
            .await?;
        let mut all: Vec<KeywordFileChunkHit> = batches.into_iter().flatten().collect();
        // Reciprocal-rank fusion avoids comparing scores from separate deal corpora. Chunk IDs are globally unique.
        all.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(a.chunk.chunk_id.cmp(&b.chunk.chunk_id))
        });
        let mut seen = std::collections::HashSet::new();
        all.retain(|h| seen.insert(h.chunk.chunk_id.clone()));
        all.truncate(search.limit);
        let authorized = access
            .current_result_ids(
                actor,
                all.iter().map(|hit| hit.chunk.chunk_id.clone()).collect(),
            )
            .await?;
        all.retain(|hit| authorized.contains(&hit.chunk.chunk_id));
        Ok(all)
    }
}
