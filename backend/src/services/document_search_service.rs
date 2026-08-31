use crate::{
    core::helix_queries::files::search_quarry_file::{
        FileChunkKeywordSearch, FileChunkVectorSearch, KeywordFileChunkHit, VectorFileChunkHit,
        MAX_FILE_CHUNK_SEARCH_LIMIT,
    },
    repository::document_repository::DocumentIndexRepository,
    services::error::{ServiceError, ServiceResult},
    utils::require_non_empty,
};

#[derive(Clone)]
pub struct DocumentSearchService {
    index: DocumentIndexRepository,
}

impl DocumentSearchService {
    pub fn new(index: DocumentIndexRepository) -> Self {
        Self { index }
    }

    pub async fn vector(
        &self,
        search: FileChunkVectorSearch,
    ) -> ServiceResult<Vec<VectorFileChunkHit>> {
        validate_common(&search.workspace_id, search.limit)?;
        if search.query_embedding.is_empty() {
            return Err(ServiceError::validation("queryEmbedding cannot be empty"));
        }
        if search
            .query_embedding
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(ServiceError::validation(
                "queryEmbedding must contain only finite values",
            ));
        }
        self.index.search_vector(search).await.map_err(Into::into)
    }

    pub async fn keyword(
        &self,
        search: FileChunkKeywordSearch,
    ) -> ServiceResult<Vec<KeywordFileChunkHit>> {
        validate_common(&search.workspace_id, search.limit)?;
        if search.query_text.trim().is_empty() {
            return Err(ServiceError::validation("queryText cannot be empty"));
        }
        self.index.search_keyword(search).await.map_err(Into::into)
    }
}

fn validate_common(workspace_id: &str, limit: usize) -> ServiceResult<()> {
    require_non_empty(workspace_id, "workspaceId").map_err(ServiceError::validation)?;
    if limit == 0 {
        return Err(ServiceError::validation("limit must be greater than zero"));
    }
    if limit > MAX_FILE_CHUNK_SEARCH_LIMIT {
        return Err(ServiceError::validation(format!(
            "limit must not exceed {MAX_FILE_CHUNK_SEARCH_LIMIT}"
        )));
    }
    Ok(())
}
