use crate::{
    domains::documents::index::{
        query::{
            FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
        },
        repository::DocumentSearchIndex,
    },
    shared::error::ServiceResult,
};

#[derive(Clone)]
pub struct DocumentSearchService {
    index: DocumentSearchIndex,
}

impl DocumentSearchService {
    pub fn new(index: DocumentSearchIndex) -> Self {
        Self { index }
    }

    pub async fn vector(
        &self,
        search: FileChunkVectorSearch,
    ) -> ServiceResult<Vec<VectorFileChunkHit>> {
        self.index.search_vector(search).await.map_err(Into::into)
    }

    pub async fn keyword(
        &self,
        search: FileChunkKeywordSearch,
    ) -> ServiceResult<Vec<KeywordFileChunkHit>> {
        self.index.search_keyword(search).await.map_err(Into::into)
    }
}
