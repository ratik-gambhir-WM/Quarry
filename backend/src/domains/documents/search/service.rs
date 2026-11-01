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
#[derive(Clone)]
pub struct DocumentSearchService {
    graph: DocumentGraphSearch,
    access: Option<ResourceAccess>,
}
impl DocumentSearchService {
    pub fn new(graph: DocumentGraphSearch) -> Self {
        Self {
            graph,
            access: None,
        }
    }
    pub fn with_access(mut self, access: ResourceAccess) -> Self {
        self.access = Some(access);
        self
    }
    fn access(&self) -> ServiceResult<&ResourceAccess> {
        self.access
            .as_ref()
            .ok_or_else(|| ServiceError::internal("search authorization unavailable"))
    }
    async fn deals(&self, actor: i64, deal: &str) -> ServiceResult<Vec<String>> {
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
        let mut all = Vec::new();
        for deal in self.deals(actor, &search.deal_id).await? {
            let candidates = access.current_chunks(actor, &deal).await?;
            self.graph
                .verify_candidates(
                    deal.clone(),
                    candidates.iter().map(|c| c.identity.clone()).collect(),
                )
                .await?;
            let hits = self
                .graph
                .vector_current(
                    FileChunkVectorSearch {
                        deal_id: deal.clone(),
                        ..search.clone()
                    },
                    candidates
                        .iter()
                        .map(|c| c.identity.chunk_id.clone())
                        .collect(),
                )
                .await?;
            let current = access
                .current_chunks(actor, &deal)
                .await?
                .into_iter()
                .map(|c| {
                    (
                        c.identity.chunk_id,
                        c.file_id,
                        c.version_id,
                        c.identity.index_generation,
                        c.identity.chunk_sha256,
                    )
                })
                .collect::<std::collections::HashSet<_>>();
            all.extend(hits.into_iter().filter(|h| {
                current.contains(&(
                    h.chunk.chunk_id.clone(),
                    h.chunk.file_id.clone(),
                    h.chunk.version_id.clone(),
                    h.chunk.index_generation.clone(),
                    h.chunk.chunk_sha256.clone(),
                ))
            }));
        }
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
        let mut all = Vec::new();
        for deal in self.deals(actor, &search.deal_id).await? {
            let candidates = access.current_chunks(actor, &deal).await?;
            self.graph
                .verify_candidates(
                    deal.clone(),
                    candidates.iter().map(|c| c.identity.clone()).collect(),
                )
                .await?;
            let hits = self
                .graph
                .keyword_current(
                    FileChunkKeywordSearch {
                        deal_id: deal.clone(),
                        ..search.clone()
                    },
                    candidates
                        .iter()
                        .map(|c| c.identity.chunk_id.clone())
                        .collect(),
                )
                .await?;
            let current = access
                .current_chunks(actor, &deal)
                .await?
                .into_iter()
                .map(|c| {
                    (
                        c.identity.chunk_id,
                        c.file_id,
                        c.version_id,
                        c.identity.index_generation,
                        c.identity.chunk_sha256,
                    )
                })
                .collect::<std::collections::HashSet<_>>();
            for (rank, mut hit) in hits.into_iter().enumerate() {
                if current.contains(&(
                    hit.chunk.chunk_id.clone(),
                    hit.chunk.file_id.clone(),
                    hit.chunk.version_id.clone(),
                    hit.chunk.index_generation.clone(),
                    hit.chunk.chunk_sha256.clone(),
                )) {
                    hit.score = 1.0 / (60.0 + rank as f64 + 1.0);
                    all.push(hit);
                }
            }
        }
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
