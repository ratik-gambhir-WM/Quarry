use std::{collections::HashSet, sync::Arc};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    adapters::helix::client::HelixClient,
    domains::documents::index::{
        model::{FileChunkNode, FileNode, FileVersionNode},
        query::{
            find_current_helix_document_by_content_hash as build_content_hash_lookup,
            get_current_helix_document as build_current_document_lookup,
            get_helix_document_version as build_document_version_lookup,
            get_helix_document_version_chunks as build_document_version_chunks_lookup,
            search_document_chunks_by_keyword as build_keyword_search,
            search_document_chunks_by_vector as build_vector_search, FileChunkKeywordSearch,
            FileChunkResult, FileChunkVectorSearch, HelixDocumentVersion, KeywordFileChunkHit,
            VectorFileChunkHit,
        },
        writer::{create_document_indexes, insert_file_version_graph},
    },
    shared::error::RepositoryError,
};

#[derive(Debug, Deserialize)]
struct ProjectionEnvelope<T> {
    properties: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HelixDocumentVersionResponse {
    file: ProjectionEnvelope<FileNode>,
    version: ProjectionEnvelope<FileVersionNode>,
}

#[derive(Debug, Deserialize)]
struct FileChunksResponse {
    chunks: ProjectionEnvelope<FileChunkResult>,
}

#[derive(Debug, Deserialize)]
struct VectorSearchResponse {
    chunks: ProjectionEnvelope<VectorFileChunkHit>,
}

#[derive(Debug, Deserialize)]
struct KeywordSearchResponse {
    chunks: ProjectionEnvelope<KeywordFileChunkHit>,
}

#[derive(Clone)]
pub struct DocumentIndexWriter {
    helix: Arc<HelixClient>,
}

impl DocumentIndexWriter {
    pub fn new(helix: Arc<HelixClient>) -> Self {
        Self { helix }
    }

    pub async fn initialize(&self) -> Result<(), RepositoryError> {
        ensure_document_indexes(&self.helix)
            .await
            .map(|_| ())
            .map_err(RepositoryError::storage)
    }

    pub async fn insert_graph(
        &self,
        filename: &str,
        file_size_bytes: u64,
        file_node: FileNode,
        version_node: FileVersionNode,
        chunk_nodes: Vec<FileChunkNode>,
    ) -> Result<Value, RepositoryError> {
        insert_document_graph(
            &self.helix,
            filename,
            file_size_bytes,
            file_node,
            version_node,
            chunk_nodes,
        )
        .await
        .map_err(RepositoryError::storage)
    }
}

#[derive(Clone)]
pub struct DocumentIndexReader {
    helix: Arc<HelixClient>,
}

impl DocumentIndexReader {
    pub fn new(helix: Arc<HelixClient>) -> Self {
        Self { helix }
    }

    pub async fn current_document(
        &self,
        workspace_id: &str,
        file_id: &str,
    ) -> Result<Option<HelixDocumentVersion>, RepositoryError> {
        get_current_helix_document(&self.helix, workspace_id, file_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn current_document_by_hash(
        &self,
        workspace_id: &str,
        content_sha256: &str,
    ) -> Result<Option<HelixDocumentVersion>, RepositoryError> {
        find_current_helix_document_by_content_hash(&self.helix, workspace_id, content_sha256)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn document_version(
        &self,
        workspace_id: &str,
        file_id: &str,
        version_id: &str,
    ) -> Result<Option<HelixDocumentVersion>, RepositoryError> {
        get_helix_document_version(&self.helix, workspace_id, file_id, version_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn document_version_chunks(
        &self,
        workspace_id: &str,
        file_id: &str,
        version_id: &str,
    ) -> Result<Vec<FileChunkResult>, RepositoryError> {
        get_helix_document_version_chunks(&self.helix, workspace_id, file_id, version_id)
            .await
            .map_err(RepositoryError::storage)
    }
}

#[derive(Clone)]
pub struct DocumentSearchIndex {
    helix: Arc<HelixClient>,
}

impl DocumentSearchIndex {
    pub fn new(helix: Arc<HelixClient>) -> Self {
        Self { helix }
    }

    pub async fn search_vector(
        &self,
        search: FileChunkVectorSearch,
    ) -> Result<Vec<VectorFileChunkHit>, RepositoryError> {
        search_document_chunks_by_vector(&self.helix, search)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn search_keyword(
        &self,
        search: FileChunkKeywordSearch,
    ) -> Result<Vec<KeywordFileChunkHit>, RepositoryError> {
        search_document_chunks_by_keyword(&self.helix, search)
            .await
            .map_err(RepositoryError::storage)
    }
}

async fn insert_document_graph(
    helix: &HelixClient,
    filename: &str,
    file_size_bytes: u64,
    file_node: FileNode,
    version_node: FileVersionNode,
    chunk_nodes: Vec<FileChunkNode>,
) -> Result<Value, String> {
    let query = insert_file_version_graph(file_node, version_node, chunk_nodes)?;
    helix
        .execute_document_query(
            "helix.file_version.insert",
            filename,
            file_size_bytes,
            move || query,
        )
        .await
}

async fn ensure_document_indexes(helix: &HelixClient) -> Result<Value, String> {
    helix.execute_dynamic_query(create_document_indexes).await
}

async fn find_current_helix_document_by_content_hash(
    helix: &HelixClient,
    workspace_id: &str,
    content_sha256: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_content_hash_lookup(workspace_id.to_string(), content_sha256.to_string())?;
    let response: HelixDocumentVersionResponse = helix.execute_dynamic_query(move || query).await?;
    map_document_version_response(response, workspace_id, None, None, Some(content_sha256))
}

async fn get_current_helix_document(
    helix: &HelixClient,
    workspace_id: &str,
    file_id: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_current_document_lookup(workspace_id.to_string(), file_id.to_string())?;
    let response: HelixDocumentVersionResponse = helix.execute_dynamic_query(move || query).await?;
    map_document_version_response(response, workspace_id, Some(file_id), None, None)
}

async fn get_helix_document_version(
    helix: &HelixClient,
    workspace_id: &str,
    file_id: &str,
    version_id: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_document_version_lookup(
        workspace_id.to_string(),
        file_id.to_string(),
        version_id.to_string(),
    )?;
    let response: HelixDocumentVersionResponse = helix.execute_dynamic_query(move || query).await?;
    map_document_version_response(
        response,
        workspace_id,
        Some(file_id),
        Some(version_id),
        None,
    )
}

async fn get_helix_document_version_chunks(
    helix: &HelixClient,
    workspace_id: &str,
    file_id: &str,
    version_id: &str,
) -> Result<Vec<FileChunkResult>, String> {
    let query = build_document_version_chunks_lookup(
        workspace_id.to_string(),
        file_id.to_string(),
        version_id.to_string(),
    )?;
    let response: FileChunksResponse = helix.execute_dynamic_query(move || query).await?;
    let mut chunks = response.chunks.properties;
    let mut indices = HashSet::with_capacity(chunks.len());
    for chunk in &chunks {
        if chunk.workspace_id != workspace_id
            || chunk.file_id != file_id
            || chunk.version_id != version_id
        {
            return Err(
                "Helix version-chunk response contained a mismatched graph identity".to_string(),
            );
        }
        if !indices.insert(chunk.chunk_index) {
            return Err(format!(
                "Helix version-chunk response contained duplicate chunk index {}",
                chunk.chunk_index
            ));
        }
    }
    chunks.sort_by_key(|chunk| chunk.chunk_index);
    Ok(chunks)
}

async fn search_document_chunks_by_vector(
    helix: &HelixClient,
    search: FileChunkVectorSearch,
) -> Result<Vec<VectorFileChunkHit>, String> {
    let workspace_id = search.workspace_id.clone();
    let query = build_vector_search(search)?;
    let response: VectorSearchResponse = helix.execute_dynamic_query(move || query).await?;
    validate_search_identities(
        &workspace_id,
        response.chunks.properties.iter().map(|hit| &hit.chunk),
    )?;
    Ok(response.chunks.properties)
}

async fn search_document_chunks_by_keyword(
    helix: &HelixClient,
    search: FileChunkKeywordSearch,
) -> Result<Vec<KeywordFileChunkHit>, String> {
    let workspace_id = search.workspace_id.clone();
    let query = build_keyword_search(search)?;
    let response: KeywordSearchResponse = helix.execute_dynamic_query(move || query).await?;
    validate_search_identities(
        &workspace_id,
        response.chunks.properties.iter().map(|hit| &hit.chunk),
    )?;
    Ok(response.chunks.properties)
}

pub(crate) fn map_document_version_response(
    response: HelixDocumentVersionResponse,
    workspace_id: &str,
    expected_file_id: Option<&str>,
    expected_version_id: Option<&str>,
    expected_content_sha256: Option<&str>,
) -> Result<Option<HelixDocumentVersion>, String> {
    let files = response.file.properties;
    let versions = response.version.properties;
    if files.is_empty() && versions.is_empty() {
        return Ok(None);
    }
    if files.len() != 1 || versions.len() != 1 {
        return Err(format!(
            "Helix document response integrity error: expected one file and one version, received {} file(s) and {} version(s)",
            files.len(),
            versions.len()
        ));
    }
    let file = files.into_iter().next().expect("one file was checked");
    let version = versions
        .into_iter()
        .next()
        .expect("one version was checked");
    if file.workspace_id != workspace_id
        || version.workspace_id != workspace_id
        || version.file_id != file.file_id
        || expected_file_id.is_some_and(|expected| file.file_id != expected)
        || expected_version_id.is_some_and(|expected| version.version_id != expected)
        || expected_content_sha256.is_some_and(|expected| version.content_sha256 != expected)
    {
        return Err("Helix document response contained a mismatched graph identity".to_string());
    }
    Ok(Some(HelixDocumentVersion { file, version }))
}

fn validate_search_identities<'a>(
    workspace_id: &str,
    chunks: impl Iterator<Item = &'a FileChunkResult>,
) -> Result<(), String> {
    for chunk in chunks {
        if chunk.workspace_id != workspace_id {
            return Err(format!(
                "Helix search response chunk `{}` belongs to workspace `{}`, not `{workspace_id}`",
                chunk.chunk_id, chunk.workspace_id
            ));
        }
    }
    Ok(())
}
