use super::fusion::sort_identity;
use crate::{
    domains::{
        documents::store::{
            chunk::{
                FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit,
                VectorFileChunkHit,
            },
            search_repository::DocumentSearchRepository,
        },
        identity::access::ResourceAccess,
        memberships::model::Capability,
    },
    shared::error::{ServiceError, ServiceResult},
};
use futures_util::{stream, StreamExt, TryStreamExt};
#[derive(Clone)]
pub struct DocumentSearchService {
    pub(super) repository: DocumentSearchRepository,
    reads: std::sync::Arc<tokio::sync::Semaphore>,
    pub(super) hybrid: std::sync::Arc<super::hybrid::HybridRuntime>,
    access: Option<ResourceAccess>,
}
impl DocumentSearchService {
    pub fn with_hybrid(
        mut self,
        config: crate::app::config::SearchConfig,
        embeddings: Option<std::sync::Arc<super::embedding::QueryEmbeddings>>,
    ) -> Self {
        self.reads = std::sync::Arc::new(tokio::sync::Semaphore::new(config.sql_reads));
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
        let _permit = super::hybrid::phase("sql_queue", async {
            self.reads
                .acquire()
                .await
                .map_err(|_| ServiceError::unavailable("search unavailable"))
        })
        .await?;
        super::hybrid::phase("sql_read", async { work.await.map_err(ServiceError::from) }).await
    }

    pub(crate) fn new(repository: DocumentSearchRepository) -> Self {
        Self {
            repository,
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
        let deals = self.deals(actor, &search.deal_id).await?;
        let batches = stream::iter(deals)
            .map(|deal| {
                let vector = search.query_embedding.clone();
                async move {
                    self.read(self.repository.branches(
                        actor,
                        &deal,
                        None,
                        Some(vector),
                        search.limit,
                    ))
                    .await
                    .map(|(_, v)| v)
                }
            })
            .buffer_unordered(self.hybrid.config.deal_concurrency)
            .try_collect::<Vec<_>>()
            .await?;
        let mut hits: Vec<_> = batches.into_iter().flatten().collect();
        hits.sort_by(|a, b| {
            a.distance
                .total_cmp(&b.distance)
                .then_with(|| sort_identity(&a.chunk).cmp(&sort_identity(&b.chunk)))
        });
        hits.dedup_by(|a, b| a.chunk.chunk_id == b.chunk.chunk_id);
        hits.truncate(search.limit);
        let allowed = self
            .access()?
            .current_result_ids(
                actor,
                hits.iter().map(|h| h.chunk.chunk_id.clone()).collect(),
            )
            .await?;
        hits.retain(|hit| allowed.contains(&hit.chunk.chunk_id));
        Ok(hits)
    }
    pub async fn keyword_owned(
        &self,
        search: FileChunkKeywordSearch,
        actor: i64,
    ) -> ServiceResult<Vec<KeywordFileChunkHit>> {
        let deals = self.deals(actor, &search.deal_id).await?;
        let batches = stream::iter(deals)
            .map(|deal| {
                let query = &search.query_text;
                async move {
                    let (mut keyword, _) = self
                        .read(self.repository.branches(
                            actor,
                            &deal,
                            Some(query),
                            None,
                            search.limit,
                        ))
                        .await?;
                    for (rank, hit) in keyword.iter_mut().enumerate() {
                        hit.score = 1.0
                            / (61.0
                                + f64::from(
                                    u32::try_from(rank)
                                        .map_err(|_| ServiceError::internal("rank overflow"))?,
                                ));
                    }
                    Ok::<_, ServiceError>(keyword)
                }
            })
            .buffer_unordered(self.hybrid.config.deal_concurrency)
            .try_collect::<Vec<_>>()
            .await?;
        let mut hits: Vec<_> = batches.into_iter().flatten().collect();
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| sort_identity(&a.chunk).cmp(&sort_identity(&b.chunk)))
        });
        hits.dedup_by(|a, b| a.chunk.chunk_id == b.chunk.chunk_id);
        hits.truncate(search.limit);
        let allowed = self
            .access()?
            .current_result_ids(
                actor,
                hits.iter().map(|h| h.chunk.chunk_id.clone()).collect(),
            )
            .await?;
        hits.retain(|hit| allowed.contains(&hit.chunk.chunk_id));
        Ok(hits)
    }
}
