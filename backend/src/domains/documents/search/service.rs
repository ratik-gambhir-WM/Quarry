use crate::{
    domains::documents::graph::{
        query::{
            FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
        },
        repository::DocumentGraphSearch,
    },
    shared::error::ServiceResult,
};

#[derive(Clone)]
pub struct DocumentSearchService {
    graph: DocumentGraphSearch,
    access: Option<crate::domains::identity::access::ResourceAccess>,
}

impl DocumentSearchService {
    pub fn new(graph: DocumentGraphSearch) -> Self {
        Self {
            graph,
            access: None,
        }
    }

    pub fn with_access(mut self, access: crate::domains::identity::access::ResourceAccess) -> Self {
        self.access = Some(access);
        self
    }
    pub async fn vector_owned(
        &self,
        search: FileChunkVectorSearch,
        owner: i64,
    ) -> ServiceResult<Vec<VectorFileChunkHit>> {
        let mut result = Vec::new();
        let access = self.access.as_ref().ok_or_else(|| {
            crate::shared::error::ServiceError::internal("search authorization unavailable")
        })?;
        for hit in self.vector(search).await? {
            if access
                .live_chunk(
                    owner,
                    hit.chunk.file_id.clone(),
                    hit.chunk.version_id.clone(),
                )
                .await?
            {
                result.push(hit);
            }
        }
        Ok(result)
    }
    pub async fn keyword_owned(
        &self,
        search: FileChunkKeywordSearch,
        owner: i64,
    ) -> ServiceResult<Vec<KeywordFileChunkHit>> {
        let mut result = Vec::new();
        let access = self.access.as_ref().ok_or_else(|| {
            crate::shared::error::ServiceError::internal("search authorization unavailable")
        })?;
        for hit in self.keyword(search).await? {
            if access
                .live_chunk(
                    owner,
                    hit.chunk.file_id.clone(),
                    hit.chunk.version_id.clone(),
                )
                .await?
            {
                result.push(hit);
            }
        }
        Ok(result)
    }
    pub async fn vector(
        &self,
        search: FileChunkVectorSearch,
    ) -> ServiceResult<Vec<VectorFileChunkHit>> {
        self.graph.search_vector(search).await.map_err(Into::into)
    }

    pub async fn keyword(
        &self,
        search: FileChunkKeywordSearch,
    ) -> ServiceResult<Vec<KeywordFileChunkHit>> {
        self.graph.search_keyword(search).await.map_err(Into::into)
    }
}
