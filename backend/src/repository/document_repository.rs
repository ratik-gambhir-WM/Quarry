use serde_json::Value;

use crate::core::{
    clients::{helix::HelixClient, sqlite::SqliteClient},
    helix_queries::files::{
        insert_quarry_file::{
            create_document_indexes, insert_chunk_batches, insert_quarry_file,
            mark_quarry_file_ingestion_complete as build_ingestion_complete_query,
        },
        search_quarry_file::{
            find_quarry_file_by_content_hash, search_chunks_by_keyword, search_chunks_by_vector,
            ChunkKeywordSearch, ChunkVectorSearch,
        },
    },
    nodes::document_node::{ChunkNode, DocumentNode},
    sqlbuilder::SqlBuilder,
};
use crate::utils::{document_id_from_content, sha256_hex};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct QuarryFileLookupResponse {
    #[serde(alias = "quarryFile")]
    quarry_file: QuarryFileProperties,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct QuarryFileProperties {
    properties: Vec<QuarryFileIdentity>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct QuarryFileIdentity {
    #[serde(alias = "documentId")]
    document_id: String,
}

#[derive(Debug)]
pub struct PersistedDocumentGraph {
    pub quarry_file: Value,
    pub chunks: Vec<Value>,
}

pub async fn persist_quarry_file(
    helix: &HelixClient,
    document: DocumentNode,
) -> Result<Value, String> {
    let filename = document.file_name.clone();
    let file_size_bytes = document.file_size_bytes;
    let query = insert_quarry_file(document)?;
    helix
        .execute_document_query(
            "helix.document.insert",
            &filename,
            file_size_bytes,
            move || query,
        )
        .await
}

pub async fn mark_quarry_file_ingestion_complete(
    helix: &HelixClient,
    document: &DocumentNode,
) -> Result<Value, String> {
    let document_id = document.document_id.clone();
    let filename = document.file_name.clone();
    let file_size_bytes = document.file_size_bytes;
    let query = build_ingestion_complete_query(document_id.clone(), document.user_id.clone())?;
    helix
        .execute_document_query(
            "helix.document.complete",
            &filename,
            file_size_bytes,
            move || query,
        )
        .await
}

pub async fn persist_chunks_for_document(
    helix: &HelixClient,
    document: &DocumentNode,
    chunks: Vec<ChunkNode>,
) -> Result<Vec<Value>, String> {
    for chunk in &chunks {
        validate_document_chunk_relationship(document, chunk)?;
    }

    let queries = insert_chunk_batches(&chunks)?;
    let mut persisted = Vec::with_capacity(queries.len());
    for query in queries {
        persisted.push(
            helix
                .execute_document_query(
                    "helix.chunk.insert_batch",
                    &document.file_name,
                    document.file_size_bytes,
                    move || query,
                )
                .await?,
        );
    }
    Ok(persisted)
}

pub async fn persist_file_blob(
    sqlite: &SqliteClient,
    document: &DocumentNode,
    file_bytes: Vec<u8>,
) -> Result<String, String> {
    if file_bytes.is_empty() {
        return Err("file bytes cannot be empty".to_string());
    }

    let content_hash = sha256_hex(&file_bytes);
    if content_hash != document.content_hash {
        return Err(format!(
            "file bytes do not match content hash for document `{}`",
            document.document_id
        ));
    }

    let file_id = document_id_from_content(&document.user_id, &content_hash);
    if file_id != document.document_id {
        return Err(format!(
            "file bytes do not match document id `{}`",
            document.document_id
        ));
    }

    let query = SqlBuilder::insert_into("quarry_file_blobs")
        .value("file_id", &file_id)
        .value("file_bytes", file_bytes)
        .on_conflict_do_nothing(["file_id"])
        .build()
        .map_err(|error| format!("failed to build file blob insert: {error}"))?;

    sqlite
        .write_async(query)
        .await
        .map_err(|error| format!("failed to persist file blob: {error}"))?;

    Ok(file_id)
}

pub async fn persist_document_and_chunks(
    sqlite: &SqliteClient,
    helix: &HelixClient,
    document: DocumentNode,
    chunks: Vec<ChunkNode>,
    file_bytes: Vec<u8>,
) -> Result<PersistedDocumentGraph, String> {
    for chunk in &chunks {
        validate_document_chunk_relationship(&document, chunk)?;
    }
   // persist_file_blob(sqlite, &document, file_bytes).await?;

    let quarry_file = persist_quarry_file(helix, document.clone()).await?;
    let chunks = persist_chunks_for_document(helix, &document, chunks).await?;
    mark_quarry_file_ingestion_complete(helix, &document).await?;
    Ok(PersistedDocumentGraph {
        quarry_file,
        chunks,
    })
}

pub async fn ensure_document_indexes(helix: &HelixClient) -> Result<Value, String> {
    helix.execute_dynamic_query(create_document_indexes).await
}

// Fix the function below to find doucment
pub async fn find_existing_document_id_by_content_hash(
    helix: &HelixClient,
    user_id: &str,
    content_hash: &str,
    filename: &str,
    file_size_bytes: u64,
) -> Result<Option<String>, String> {
    let query = find_quarry_file_by_content_hash(user_id.to_string(), content_hash.to_string())?;
    let response: QuarryFileLookupResponse = helix
        .execute_document_query(
            "helix.document.lookup",
            filename,
            file_size_bytes,
            move || query,
        )
        .await?;
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
) -> Result<Value, String> {
    let query = search_chunks_by_vector(search)?;
    helix.execute_dynamic_query(move || query).await
}

pub async fn search_document_chunks_by_keyword(
    helix: &HelixClient,
    search: ChunkKeywordSearch,
) -> Result<Value, String> {
    let query = search_chunks_by_keyword(search)?;
    helix.execute_dynamic_query(move || query).await
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
