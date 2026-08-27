use std::{collections::HashSet, path::Path};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::core::{
    clients::{
        helix::HelixClient,
        sqlite::{SqlRow, SqliteClient, SqliteClientError, SqliteTransaction},
    },
    helix_queries::files::{
        insert_quarry_file::{create_document_indexes, insert_file_version_graph},
        search_quarry_file::{
            find_current_helix_document_by_content_hash as build_content_hash_lookup,
            get_current_helix_document as build_current_document_lookup,
            get_helix_document_version as build_document_version_lookup,
            get_helix_document_version_chunks as build_document_version_chunks_lookup,
            search_document_chunks_by_keyword as build_keyword_search,
            search_document_chunks_by_vector as build_vector_search, FileChunkKeywordSearch,
            FileChunkResult, FileChunkVectorSearch, HelixDocumentVersion, KeywordFileChunkHit,
            VectorFileChunkHit,
        },
    },
    models::file_persistence::{ExistingFileVersion, FilePersistenceInput, PersistedFileIdentity},
    nodes::document_node::{FileChunkNode, FileNode, FileVersionNode},
    sqlbuilder::{Condition, ConflictUpdate, SortDirection, SqlBuilder, SqlQuery, SqlValue},
};
use crate::{
    core::infer_supported_mime_type,
    services::document_ingestion_service::{Document, DocumentChunk},
    utils::{document_id_from_content, file_version_id, sha256_hex},
};

#[derive(Debug, Deserialize)]
struct ProjectionEnvelope<T> {
    properties: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct HelixDocumentVersionResponse {
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

#[derive(Debug)]
pub struct PersistedDocumentGraph {
    pub persisted: PersistedFileIdentity,
    pub helix_transaction: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealDocumentSummary {
    pub file_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocumentBlob {
    pub file_id: String,
    pub display_name: String,
    pub mime_type: String,
    pub file_bytes: Vec<u8>,
}

pub async fn list_current_deal_documents(
    sqlite: &SqliteClient,
    deal_id: &str,
) -> Result<Vec<DealDocumentSummary>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns(["quarry_files.file_id", "quarry_files.display_name"])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        .and_where(Condition::equal("quarry_files.deal_id", deal_id))
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .order_by("quarry_files.display_name", SortDirection::Ascending)
        .order_by("quarry_files.file_id", SortDirection::Ascending)
        .build()
        .map_err(|error| format!("failed to build deal document list query: {error}"))?;
    let rows = sqlite
        .read_async(query)
        .await
        .map_err(|error| format!("failed to list deal documents: {error}"))?;

    rows.into_iter()
        .map(|row| {
            Ok(DealDocumentSummary {
                file_id: required_text(&row, "file_id")?,
                display_name: required_text(&row, "display_name")?,
            })
        })
        .collect::<Result<Vec<_>, SqliteClientError>>()
        .map_err(|error| format!("failed to decode deal document list: {error}"))
}

pub async fn get_current_deal_document_blob(
    sqlite: &SqliteClient,
    deal_id: &str,
    file_id: &str,
) -> Result<Option<StoredDocumentBlob>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.display_name",
            "quarry_file_versions.mime_type",
            "quarry_file_blobs.file_bytes",
        ])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        .inner_join(
            "quarry_file_blobs",
            "quarry_file_versions.version_id",
            "quarry_file_blobs.version_id",
        )
        .and_where(Condition::equal("quarry_files.deal_id", deal_id))
        .and_where(Condition::equal("quarry_files.file_id", file_id))
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .build()
        .map_err(|error| format!("failed to build document blob query: {error}"))?;
    let row = sqlite
        .read_one_async(query)
        .await
        .map_err(|error| format!("failed to load document blob: {error}"))?;

    row.map(|row| {
        Ok(StoredDocumentBlob {
            file_id: required_text(&row, "file_id")?,
            display_name: required_text(&row, "display_name")?,
            mime_type: required_text(&row, "mime_type")?,
            file_bytes: required_blob(&row, "file_bytes")?,
        })
    })
    .transpose()
    .map_err(|error: SqliteClientError| format!("failed to decode document blob: {error}"))
}

pub async fn find_current_sqlite_file_by_content_hash(
    sqlite: &SqliteClient,
    deal_id: &str,
    workspace_id: &str,
    content_sha256: &str,
) -> Result<Option<PersistedFileIdentity>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.workspace_id",
            "quarry_files.display_name",
            "quarry_file_versions.version_id",
        ])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        .inner_join(
            "quarry_file_blobs",
            "quarry_file_versions.version_id",
            "quarry_file_blobs.version_id",
        )
        .and_where(Condition::equal("quarry_files.deal_id", deal_id))
        .and_where(Condition::equal("quarry_files.workspace_id", workspace_id))
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .and_where(Condition::equal(
            "quarry_file_versions.content_sha256",
            content_sha256,
        ))
        .build()
        .map_err(|error| format!("failed to build deal attachment lookup: {error}"))?;
    let rows = sqlite
        .read_async(query)
        .await
        .map_err(|error| format!("failed to find deal attachment: {error}"))?;
    if rows.len() > 1 {
        return Err(format!(
            "deal `{deal_id}` contains multiple current attachments for content hash `{content_sha256}`"
        ));
    }

    rows.into_iter()
        .next()
        .map(|row| {
            Ok(PersistedFileIdentity {
                file_id: required_text(&row, "file_id")?,
                workspace_id: required_text(&row, "workspace_id")?,
                display_name: required_text(&row, "display_name")?,
                version_id: required_text(&row, "version_id")?,
            })
        })
        .transpose()
        .map_err(|error: SqliteClientError| format!("failed to decode deal attachment: {error}"))
}

pub async fn persist_document_and_chunks(
    sqlite: &SqliteClient,
    helix: &HelixClient,
    deal_id: &str,
    document: Document,
    chunks: Vec<DocumentChunk>,
    file_bytes: Vec<u8>,
) -> Result<PersistedDocumentGraph, String> {
    validate_document_chunks(&document, &chunks)?;
    let filename = document.file_name.clone();
    let file_size_bytes = document.file_size_bytes;
    let persisted = persist_file_blob(sqlite, deal_id, &document, file_bytes).await?;
    let (file, version, chunks) = graph_nodes_from_persisted(&persisted, &document, &chunks)?;
    let query = insert_file_version_graph(file, version, chunks)?;
    let helix_transaction = helix
        .execute_document_query(
            "helix.file_version.insert",
            &filename,
            file_size_bytes,
            move || query,
        )
        .await
        .map_err(|error| {
            format!(
                "Helix indexing failed after SQLite committed file_id `{}` and version_id `{}`: {error}",
                persisted.file_id, persisted.version_id
            )
        })?;
    Ok(PersistedDocumentGraph {
        persisted,
        helix_transaction,
    })
}

pub async fn persist_file_blob(
    sqlite: &SqliteClient,
    deal_id: &str,
    document: &Document,
    file_bytes: Vec<u8>,
) -> Result<PersistedFileIdentity, String> {
    let input = validate_and_build_file_persistence(deal_id, document, file_bytes)?;
    sqlite
        .transaction_async(move |transaction| {
            validate_deal_ownership(transaction, &input)?;

            // The aggregate is always mutated parent-to-child.
            validate_existing_quarry_file(transaction, &input)?;
            upsert_quarry_file(transaction, &input)?;

            if let Some(existing) = find_existing_file_version(transaction, &input)? {
                verify_idempotent_version_and_blob(transaction, &input, &existing)?;
                make_existing_version_current(transaction, &input, &existing)?;
            } else {
                let version_number = next_file_version_number(transaction, &input)?;

                clear_current_file_version(transaction, &input)?;
                insert_quarry_file_version(transaction, &input, version_number)?;
                insert_quarry_file_blob(transaction, &input)?;
            }

            read_back_persisted_file_identity(transaction, &input)
        })
        .await
        .map_err(|error| format!("failed to persist file transaction: {error}"))
}

fn graph_nodes_from_persisted(
    persisted: &PersistedFileIdentity,
    document: &Document,
    chunks: &[DocumentChunk],
) -> Result<(FileNode, FileVersionNode, Vec<FileChunkNode>), String> {
    validate_document_chunks(document, chunks)?;
    if persisted.workspace_id != document.user_id
        || persisted.file_id != document.file_id
        || persisted.display_name != document.file_name
    {
        return Err(
            "committed SQLite file identity does not match the ingestion document".to_string(),
        );
    }
    let expected_version_id = file_version_id(&document.file_id, &document.content_hash);
    if persisted.version_id != expected_version_id {
        return Err(
            "committed SQLite version identity does not match the ingestion document".to_string(),
        );
    }

    let mime_type = infer_supported_mime_type(Path::new(&document.file_name))
        .ok_or_else(|| format!("unsupported file type for `{}`", document.file_name))?
        .to_string();
    let byte_size = i64::try_from(document.file_size_bytes)
        .map_err(|_| "document byte size does not fit in i64".to_string())?;
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let index_generation = persisted.version_id.clone();
    let file = FileNode {
        workspace_id: persisted.workspace_id.clone(),
        file_id: persisted.file_id.clone(),
        display_name: persisted.display_name.clone(),
    };
    let version = FileVersionNode {
        workspace_id: persisted.workspace_id.clone(),
        file_id: persisted.file_id.clone(),
        version_id: persisted.version_id.clone(),
        mime_type,
        content_sha256: document.content_hash.clone(),
        byte_size,
        index_generation: index_generation.clone(),
        indexed_at: timestamp.clone(),
    };
    let graph_chunks = chunks
        .iter()
        .map(|chunk| {
            let chunk_index = i64::from(chunk.sequence_number);
            let (page_start, page_end) = page_range(chunk)?;
            Ok(FileChunkNode {
                chunk_id: deterministic_file_chunk_id(
                    &persisted.workspace_id,
                    &persisted.file_id,
                    &persisted.version_id,
                    &index_generation,
                    chunk_index,
                    &chunk.content_hash,
                ),
                workspace_id: persisted.workspace_id.clone(),
                file_id: persisted.file_id.clone(),
                version_id: persisted.version_id.clone(),
                index_generation: index_generation.clone(),
                chunk_index,
                text: chunk.text.clone(),
                embedding: chunk.embedding.clone().ok_or_else(|| {
                    format!("chunk `{}` does not contain an embedding", chunk.chunk_id)
                })?,
                chunk_sha256: chunk.content_hash.clone(),
                token_count: i64::from(chunk.token_count),
                page_start,
                page_end,
                char_start: usize_to_i64(chunk.start_offset, "char_start")?,
                char_end: usize_to_i64(chunk.end_offset, "char_end")?,
                section_path: chunk.section_title.clone().unwrap_or_default(),
                created_at: timestamp.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((file, version, graph_chunks))
}

pub fn deterministic_file_chunk_id(
    workspace_id: &str,
    file_id: &str,
    version_id: &str,
    index_generation: &str,
    chunk_index: i64,
    chunk_sha256: &str,
) -> String {
    sha256_hex(
        format!(
            "{workspace_id}\0{file_id}\0{version_id}\0{index_generation}\0{chunk_index}\0{chunk_sha256}"
        )
        .as_bytes(),
    )
}

fn validate_document_chunks(document: &Document, chunks: &[DocumentChunk]) -> Result<(), String> {
    let mut chunk_indices = HashSet::with_capacity(chunks.len());
    let mut embedding_dimension = None;
    for chunk in chunks {
        validate_document_chunk_relationship(document, chunk)?;
        if !chunk_indices.insert(chunk.sequence_number) {
            return Err(format!(
                "document contains duplicate chunk index {}",
                chunk.sequence_number
            ));
        }
        if chunk.start_offset > chunk.end_offset {
            return Err(format!(
                "chunk `{}` has an invalid character range",
                chunk.chunk_id
            ));
        }
        let embedding = chunk
            .embedding
            .as_ref()
            .ok_or_else(|| format!("chunk `{}` does not contain an embedding", chunk.chunk_id))?;
        if embedding.is_empty() {
            return Err(format!("chunk `{}` has an empty embedding", chunk.chunk_id));
        }
        if embedding.iter().any(|value| !value.is_finite()) {
            return Err(format!(
                "chunk `{}` embedding must contain only finite values",
                chunk.chunk_id
            ));
        }
        match embedding_dimension {
            Some(expected) if expected != embedding.len() => {
                return Err(format!(
                    "chunk `{}` embedding dimension {} does not match expected dimension {expected}",
                    chunk.chunk_id,
                    embedding.len()
                ));
            }
            None => embedding_dimension = Some(embedding.len()),
            _ => {}
        }
        page_range(chunk)?;
        usize_to_i64(chunk.start_offset, "char_start")?;
        usize_to_i64(chunk.end_offset, "char_end")?;
    }
    Ok(())
}

fn page_range(chunk: &DocumentChunk) -> Result<(Option<i64>, Option<i64>), String> {
    let Some(page_numbers) = chunk
        .page_numbers
        .as_ref()
        .filter(|pages| !pages.is_empty())
    else {
        return Ok((None, None));
    };
    let minimum = page_numbers
        .iter()
        .min()
        .copied()
        .ok_or_else(|| format!("chunk `{}` has no minimum page", chunk.chunk_id))?;
    let maximum = page_numbers
        .iter()
        .max()
        .copied()
        .ok_or_else(|| format!("chunk `{}` has no maximum page", chunk.chunk_id))?;
    let page_start = i64::from(minimum);
    let page_end = i64::from(maximum);
    if page_start > page_end {
        return Err(format!(
            "chunk `{}` has an invalid page range",
            chunk.chunk_id
        ));
    }
    Ok((Some(page_start), Some(page_end)))
}

fn usize_to_i64(value: usize, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} value `{value}` does not fit in i64"))
}

fn validate_and_build_file_persistence(
    deal_id: &str,
    document: &Document,
    file_bytes: Vec<u8>,
) -> Result<FilePersistenceInput, String> {
    let deal_id = normalized_nonempty("deal_id", deal_id)?;
    let file_id = normalized_nonempty("file_id", &document.file_id)?;
    let workspace_id = normalized_workspace_identity(&document.user_id)?;
    let display_name = normalized_nonempty("file_name", &document.file_name)?;
    let source_type = normalized_nonempty("source_type", &document.source_type)?;
    let source_uri = document
        .local_path
        .as_deref()
        .map(|path| normalized_nonempty("local_path", path))
        .transpose()?;

    if file_bytes.is_empty() {
        return Err("file bytes cannot be empty".to_string());
    }
    let actual_size = u64::try_from(file_bytes.len())
        .map_err(|_| "file byte length does not fit in u64".to_string())?;
    if actual_size != document.file_size_bytes {
        return Err(format!(
            "file byte size {actual_size} does not match document byte size {}",
            document.file_size_bytes
        ));
    }
    let byte_size = i64::try_from(file_bytes.len())
        .map_err(|_| "file byte length does not fit in SQLite INTEGER".to_string())?;

    let content_sha256 = sha256_hex(&file_bytes);
    if content_sha256 != document.content_hash {
        return Err(format!(
            "file bytes do not match content hash for document `{}`",
            document.document_id
        ));
    }
    let expected_document_id = document_id_from_content(&workspace_id, &content_sha256);
    if expected_document_id != document.document_id {
        return Err(format!(
            "file bytes do not match document id `{}`",
            document.document_id
        ));
    }

    let mime_type = infer_supported_mime_type(Path::new(&display_name))
        .ok_or_else(|| format!("unsupported file type for `{display_name}`"))?;
    let extension = Path::new(&display_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| format!("unsupported file type for `{display_name}`"))?;
    if source_type.to_ascii_lowercase() != extension {
        return Err(format!(
            "source type `{source_type}` does not match filename `{display_name}`"
        ));
    }

    let metadata_json = serde_json::to_string(&json!({
        "documentId": document.document_id,
        "sourceType": source_type,
        "tokenCount": document.token_count,
        "renderedPdfPath": document.rendered_pdf_path,
    }))
    .map_err(|error| format!("failed to serialize file metadata: {error}"))?;
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let version_id = file_version_id(&file_id, &content_sha256);

    Ok(FilePersistenceInput {
        deal_id,
        file_id,
        workspace_id,
        display_name,
        source_uri,
        metadata_json,
        timestamp,
        version_id,
        mime_type: mime_type.to_string(),
        content_sha256,
        byte_size,
        file_bytes,
    })
}

fn normalized_nonempty(field: &str, value: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(format!("{field} cannot be empty"));
    }
    if normalized != value {
        return Err(format!("{field} must not contain surrounding whitespace"));
    }
    Ok(normalized.to_string())
}

fn normalized_workspace_identity(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("user_id cannot be empty".to_string());
    }
    if normalized != value {
        return Err("user_id must be trimmed and lowercase".to_string());
    }
    Ok(normalized)
}

fn normalize_stored_owner(value: &str) -> String {
    value.trim().to_lowercase()
}

fn validate_deal_ownership(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("deals")
            .columns(["deals.status", "users.email"])
            .inner_join("users", "deals.user_id", "users.id")
            .and_where(Condition::equal("deals.deal_id", &input.deal_id))
            .build(),
        "deal ownership select",
    )?;
    let row = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read deal ownership"))?
        .ok_or_else(|| {
            SqliteClientError::transaction_aborted(format!(
                "deal `{}` does not exist",
                input.deal_id
            ))
        })?;
    let status = required_text(&row, "status")?;
    if status.trim().eq_ignore_ascii_case("archived") {
        return Err(SqliteClientError::transaction_aborted(format!(
            "deal `{}` is archived",
            input.deal_id
        )));
    }
    let owner = normalize_stored_owner(&required_text(&row, "email")?);
    if owner != input.workspace_id {
        return Err(SqliteClientError::transaction_aborted(format!(
            "deal `{}` is not owned by workspace `{}`",
            input.deal_id, input.workspace_id
        )));
    }
    Ok(())
}

fn validate_existing_quarry_file(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_files")
            .columns(["deal_id", "workspace_id", "deleted_at"])
            .and_where(Condition::equal("file_id", &input.file_id))
            .build(),
        "existing quarry file select",
    )?;
    let Some(row) = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read existing quarry file"))?
    else {
        return Ok(());
    };

    if required_text(&row, "deal_id")? != input.deal_id {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` already belongs to another deal",
            input.file_id
        )));
    }
    if required_text(&row, "workspace_id")? != input.workspace_id {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` already belongs to another workspace",
            input.file_id
        )));
    }
    if !matches!(row.get("deleted_at"), Some(SqlValue::Null)) {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` is deleted and cannot be restored by persistence",
            input.file_id
        )));
    }
    Ok(())
}

fn upsert_quarry_file(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::insert_into("quarry_files")
            .value("file_id", &input.file_id)
            .value("deal_id", &input.deal_id)
            .value("workspace_id", &input.workspace_id)
            .value("display_name", &input.display_name)
            .value("source_uri", input.source_uri.clone())
            .value("metadata_json", &input.metadata_json)
            .value("created_at", &input.timestamp)
            .value("updated_at", &input.timestamp)
            .value("deleted_at", Option::<String>::None)
            .on_conflict_update(
                ConflictUpdate::new(["file_id"])
                    .set_excluded("display_name")
                    .set_excluded("source_uri")
                    .set_excluded("metadata_json")
                    .set_excluded("updated_at"),
            )
            .build(),
        "quarry_files upsert",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to upsert quarry file"))?
            .rows_affected,
        "quarry_files upsert",
    )
}

fn find_existing_file_version(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<Option<ExistingFileVersion>, SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_file_versions")
            .columns(["version_id", "byte_size", "is_current"])
            .and_where(Condition::equal("file_id", &input.file_id))
            .and_where(Condition::equal("content_sha256", &input.content_sha256))
            .build(),
        "existing file version select",
    )?;
    transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read existing file version"))?
        .map(|row| {
            let is_current = required_integer(&row, "is_current")?;
            if !matches!(is_current, 0 | 1) {
                return Err(SqliteClientError::transaction_aborted(
                    "stored file version has an invalid is_current value",
                ));
            }
            Ok(ExistingFileVersion {
                version_id: required_text(&row, "version_id")?,
                byte_size: required_integer(&row, "byte_size")?,
                is_current: is_current == 1,
            })
        })
        .transpose()
}

fn verify_idempotent_version_and_blob(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
    existing: &ExistingFileVersion,
) -> Result<(), SqliteClientError> {
    if existing.version_id != input.version_id || existing.byte_size != input.byte_size {
        return Err(SqliteClientError::transaction_aborted(format!(
            "stored version for file `{}` does not match its derived identity or byte size",
            input.file_id
        )));
    }
    let query = build_transaction_query(
        SqlBuilder::select("quarry_file_blobs")
            .column("file_bytes")
            .and_where(Condition::equal("version_id", &existing.version_id))
            .build(),
        "existing file blob select",
    )?;
    let row = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read existing file blob"))?
        .ok_or_else(|| {
            SqliteClientError::transaction_aborted(format!(
                "stored version `{}` has no blob",
                existing.version_id
            ))
        })?;
    if required_blob(&row, "file_bytes")? != input.file_bytes {
        return Err(SqliteClientError::transaction_aborted(format!(
            "stored blob for version `{}` is corrupt or collided",
            existing.version_id
        )));
    }
    Ok(())
}

fn make_existing_version_current(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
    existing: &ExistingFileVersion,
) -> Result<(), SqliteClientError> {
    if existing.is_current {
        return Ok(());
    }
    clear_current_file_version(transaction, input)?;
    let query = build_transaction_query(
        SqlBuilder::update("quarry_file_versions")
            .set("is_current", true)
            .and_where(Condition::equal("version_id", &existing.version_id))
            .and_where(Condition::equal("file_id", &input.file_id))
            .build(),
        "existing file version current update",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to make existing file version current"))?
            .rows_affected,
        "existing file version current update",
    )
}

fn next_file_version_number(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<i64, SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_file_versions")
            .column("version_number")
            .and_where(Condition::equal("file_id", &input.file_id))
            .order_by("version_number", SortDirection::Descending)
            .limit(1)
            .build(),
        "latest file version select",
    )?;
    let latest = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read latest file version"))?
        .map(|row| required_integer(&row, "version_number"))
        .transpose()?;
    match latest {
        Some(version) if version > 0 => version.checked_add(1).ok_or_else(|| {
            SqliteClientError::transaction_aborted(format!(
                "version number overflow for file `{}`",
                input.file_id
            ))
        }),
        Some(_) => Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` has an invalid stored version number",
            input.file_id
        ))),
        None => Ok(1),
    }
}

fn clear_current_file_version(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::update("quarry_file_versions")
            .set("is_current", false)
            .and_where(Condition::equal("file_id", &input.file_id))
            .and_where(Condition::equal("is_current", true))
            .build(),
        "current file version clear",
    )?;
    let rows_affected = transaction
        .write(&query)
        .map_err(|error| error.context("failed to clear current file version"))?
        .rows_affected;
    if rows_affected > 1 {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` had more than one current version",
            input.file_id
        )));
    }
    Ok(())
}

fn insert_quarry_file_version(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
    version_number: i64,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::insert_into("quarry_file_versions")
            .value("version_id", &input.version_id)
            .value("file_id", &input.file_id)
            .value("version_number", version_number)
            .value("original_filename", &input.display_name)
            .value("mime_type", &input.mime_type)
            .value("content_sha256", &input.content_sha256)
            .value("byte_size", input.byte_size)
            .value("is_current", true)
            .value("created_at", &input.timestamp)
            .build(),
        "quarry_file_versions insert",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to insert quarry file version"))?
            .rows_affected,
        "quarry_file_versions insert",
    )
}

fn insert_quarry_file_blob(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::insert_into("quarry_file_blobs")
            .value("version_id", &input.version_id)
            .value("file_bytes", input.file_bytes.clone())
            .build(),
        "quarry_file_blobs insert",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to insert quarry file blob"))?
            .rows_affected,
        "quarry_file_blobs insert",
    )
}

fn read_back_persisted_file_identity(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<PersistedFileIdentity, SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_files")
            .columns([
                "quarry_files.file_id",
                "quarry_files.workspace_id",
                "quarry_files.display_name",
                "quarry_file_versions.version_id",
            ])
            .inner_join(
                "quarry_file_versions",
                "quarry_files.file_id",
                "quarry_file_versions.file_id",
            )
            .and_where(Condition::equal("quarry_files.file_id", &input.file_id))
            .and_where(Condition::equal(
                "quarry_file_versions.version_id",
                &input.version_id,
            ))
            .build(),
        "persisted file identity read-back",
    )?;
    let rows = transaction
        .read(&query)
        .map_err(|error| error.context("failed to read back persisted file identity"))?;
    if rows.len() != 1 {
        return Err(SqliteClientError::transaction_aborted(format!(
            "persisted file identity read-back returned {} rows instead of one",
            rows.len()
        )));
    }
    let row = &rows[0];
    let identity = PersistedFileIdentity {
        file_id: required_text(row, "file_id")?,
        workspace_id: required_text(row, "workspace_id")?,
        display_name: required_text(row, "display_name")?,
        version_id: required_text(row, "version_id")?,
    };
    if identity.file_id != input.file_id
        || identity.workspace_id != input.workspace_id
        || identity.display_name != input.display_name
        || identity.version_id != input.version_id
    {
        return Err(SqliteClientError::transaction_aborted(
            "persisted file identity read-back did not match the validated input",
        ));
    }
    Ok(identity)
}

fn build_transaction_query(
    query: Result<SqlQuery, crate::core::sqlbuilder::SqlBuilderError>,
    operation: &str,
) -> Result<SqlQuery, SqliteClientError> {
    query.map_err(|error| {
        SqliteClientError::transaction_aborted(format!("failed to build {operation}: {error}"))
    })
}

fn require_one_row(rows_affected: usize, operation: &str) -> Result<(), SqliteClientError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(SqliteClientError::transaction_aborted(format!(
            "{operation} affected {rows_affected} rows instead of one"
        )))
    }
}

fn required_text(row: &SqlRow, column: &str) -> Result<String, SqliteClientError> {
    match row.get(column) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(invalid_row_value(column, "TEXT")),
    }
}

fn required_integer(row: &SqlRow, column: &str) -> Result<i64, SqliteClientError> {
    match row.get(column) {
        Some(SqlValue::Integer(value)) => Ok(*value),
        _ => Err(invalid_row_value(column, "INTEGER")),
    }
}

fn required_blob(row: &SqlRow, column: &str) -> Result<Vec<u8>, SqliteClientError> {
    match row.get(column) {
        Some(SqlValue::Blob(value)) => Ok(value.clone()),
        _ => Err(invalid_row_value(column, "BLOB")),
    }
}

fn invalid_row_value(column: &str, expected: &str) -> SqliteClientError {
    SqliteClientError::transaction_aborted(format!(
        "transaction row column `{column}` was missing or was not {expected}"
    ))
}

pub async fn ensure_document_indexes(helix: &HelixClient) -> Result<Value, String> {
    helix.execute_dynamic_query(create_document_indexes).await
}

pub async fn find_current_helix_document_by_content_hash(
    helix: &HelixClient,
    workspace_id: &str,
    content_sha256: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_content_hash_lookup(workspace_id.to_string(), content_sha256.to_string())?;
    let response: HelixDocumentVersionResponse = helix.execute_dynamic_query(move || query).await?;
    map_document_version_response(response, workspace_id, None, None, Some(content_sha256))
}

pub async fn get_current_helix_document(
    helix: &HelixClient,
    workspace_id: &str,
    file_id: &str,
) -> Result<Option<HelixDocumentVersion>, String> {
    let query = build_current_document_lookup(workspace_id.to_string(), file_id.to_string())?;
    let response: HelixDocumentVersionResponse = helix.execute_dynamic_query(move || query).await?;
    map_document_version_response(response, workspace_id, Some(file_id), None, None)
}

pub async fn get_helix_document_version(
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

pub async fn get_helix_document_version_chunks(
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

pub async fn search_document_chunks_by_vector(
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

pub async fn search_document_chunks_by_keyword(
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

fn validate_document_chunk_relationship(
    document: &Document,
    chunk: &DocumentChunk,
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

fn map_document_version_response(
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

#[cfg(test)]
#[path = "../../tests/repository/document_repository_tests.rs"]
mod tests;
