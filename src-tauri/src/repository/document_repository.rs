use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::{
    clients::helix::HelixClient,
    helix_queries::files::{
        insert_quarry_file::{
            create_document_indexes, insert_chunk_for_document, insert_quarry_file,
            mark_quarry_file_ingestion_complete as build_ingestion_complete_query,
        },
        search_quarry_file::{
            find_quarry_file_by_content_hash, search_chunks_by_keyword, search_chunks_by_vector,
            ChunkKeywordSearch, ChunkVectorSearch,
        },
    },
    nodes::document_node::{ChunkNode, DocumentNode},
};

#[derive(Debug, Deserialize)]
struct HelixProperties<T> {
    properties: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct QuarryFileLookupResponse {
    #[serde(alias = "quarryFile")]
    quarry_file: HelixProperties<QuarryFileIdentity>,
}

#[derive(Debug, Deserialize)]
struct QuarryFileIdentity {
    #[serde(alias = "documentId")]
    document_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentChunkSearchResult {
    #[serde(alias = "document_id")]
    pub document_id: String,
    pub text: String,
    #[serde(alias = "sequence_number")]
    pub sequence_number: u32,
    #[serde(default, alias = "page_numbers")]
    pub page_numbers: Option<Vec<u32>>,
    #[serde(default, alias = "section_title")]
    pub section_title: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub distance: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ChunkSearchResponse {
    chunks: HelixProperties<DocumentChunkSearchResult>,
}

#[derive(Debug)]
pub struct PersistedDocumentGraph {
    pub quarry_file: Value,
    pub chunks: Vec<Value>,
}

/// Creates the QuarryFile node once before chunks are added.
pub async fn persist_quarry_file(
    helix: &HelixClient,
    document: DocumentNode,
) -> Result<Value, String> {
    let query = insert_quarry_file(document)?;

    helix.execute_dynamic_query(move || query).await
}

/// Adds one chunk to an existing QuarryFile.
///
/// The Helix query resolves the original node using both `document_id` and
/// `user_id`; it creates neither the chunk nor the edge when no match exists.
pub async fn persist_chunk_for_document(
    helix: &HelixClient,
    chunk: ChunkNode,
) -> Result<Value, String> {
    let query = insert_chunk_for_document(chunk)?;

    helix.execute_dynamic_query(move || query).await
}

/// Sequentially adds all supplied chunks to an already-persisted QuarryFile.
///
/// Each chunk is its own Helix transaction so callers can use this while chunks
/// are produced. Processing stops on the first failed request.
pub async fn persist_chunks_for_document(
    helix: &HelixClient,
    document: &DocumentNode,
    chunks: Vec<ChunkNode>,
) -> Result<Vec<Value>, String> {
    let mut persisted = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        validate_document_chunk_relationship(document, &chunk)?;
        persisted.push(persist_chunk_for_document(helix, chunk).await?);
    }

    Ok(persisted)
}

pub async fn mark_quarry_file_ingestion_complete(
    helix: &HelixClient,
    document: &DocumentNode,
) -> Result<Value, String> {
    let query =
        build_ingestion_complete_query(document.document_id.clone(), document.user_id.clone())?;
    helix.execute_dynamic_query(move || query).await
}

/// Creates the QuarryFile once and then adds each chunk as it is processed.
pub async fn persist_document_and_chunks(
    helix: &HelixClient,
    document: DocumentNode,
    chunks: Vec<ChunkNode>,
) -> Result<PersistedDocumentGraph, String> {
    for chunk in &chunks {
        validate_document_chunk_relationship(&document, chunk)?;
    }

    let quarry_file = persist_quarry_file(helix, document.clone()).await?;
    let chunks = persist_chunks_for_document(helix, &document, chunks).await?;
    mark_quarry_file_ingestion_complete(helix, &document).await?;

    Ok(PersistedDocumentGraph {
        quarry_file,
        chunks,
    })
}

pub async fn find_existing_document_id_by_content_hash(
    helix: &HelixClient,
    user_id: &str,
    content_hash: &str,
) -> Result<Option<String>, String> {
    let query = find_quarry_file_by_content_hash(user_id.to_string(), content_hash.to_string())?;
    let response: QuarryFileLookupResponse = helix.execute_dynamic_query(move || query).await?;
    Ok(response
        .quarry_file
        .properties
        .into_iter()
        .next()
        .map(|document| document.document_id))
}

pub async fn search_document_chunks_by_vector(
    helix: &HelixClient,
    search: ChunkVectorSearch,
) -> Result<Vec<DocumentChunkSearchResult>, String> {
    let query = search_chunks_by_vector(search)?;
    let response: ChunkSearchResponse = helix.execute_dynamic_query(move || query).await?;
    Ok(response.chunks.properties)
}

pub async fn search_document_chunks_by_keyword(
    helix: &HelixClient,
    search: ChunkKeywordSearch,
) -> Result<Vec<DocumentChunkSearchResult>, String> {
    let query = search_chunks_by_keyword(search)?;
    let response: ChunkSearchResponse = helix.execute_dynamic_query(move || query).await?;
    Ok(response.chunks.properties)
}

/// Compatibility wrapper for callers that currently persist the first chunk
/// together with its document.
pub async fn persist_document_and_chunk(
    helix: &HelixClient,
    document: DocumentNode,
    chunk: ChunkNode,
) -> Result<PersistedDocumentGraph, String> {
    persist_document_and_chunks(helix, document, vec![chunk]).await
}

/// Executes the idempotent QuarryFile and Chunk index batch.
pub async fn ensure_document_indexes(helix: &HelixClient) -> Result<Value, String> {
    helix.execute_dynamic_query(create_document_indexes).await
}

fn validate_document_chunk_relationship(
    document: &DocumentNode,
    chunk: &ChunkNode,
) -> Result<(), String> {
    if chunk.document_id != document.document_id {
        return Err(format!(
            "chunk `{}` belongs to document `{}`, not `{}`",
            chunk.chunk_id, chunk.document_id, document.document_id
        ));
    }
    if chunk.user_id != document.user_id {
        return Err(format!(
            "chunk `{}` belongs to user `{}`, not `{}`",
            chunk.chunk_id, chunk.user_id, document.user_id
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/repository/document_repository_tests.rs"]
mod tests;
