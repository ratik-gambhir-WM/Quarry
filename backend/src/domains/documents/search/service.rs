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
}

impl DocumentSearchService {
    pub fn new(graph: DocumentGraphSearch) -> Self {
        Self { graph }
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
