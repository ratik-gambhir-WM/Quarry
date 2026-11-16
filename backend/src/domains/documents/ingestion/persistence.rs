use super::repository::Attempt;
use crate::{
    domains::documents::{
        model::{Document, DocumentChunk},
        store::{
            chunk::FileChunkResult,
            identity::{ChunkIdentity, IdentityManifest},
            model::{FilePersistenceInput, PersistedFileIdentity},
            repository::DocumentStore,
            search_repository::{EmbeddingProfile, PreparedChunks},
        },
    },
    shared::{
        error::RepositoryError,
        file_policy::{infer_supported_image_mime_type, infer_supported_mime_type},
        ids::{document_id_from_content, file_version_id, sha256_hex},
    },
};
use chrono::{SecondsFormat, Utc};
use serde_json::json;
use std::{collections::HashSet, path::Path};

pub(crate) async fn persist_document_and_chunks(
    files: &DocumentStore,
    deal_id: &str,
    actor_id: i64,
    document: Document,
    chunks: Vec<DocumentChunk>,
    file_bytes: Vec<u8>,
    attempt: Option<Attempt>,
) -> Result<PersistedFileIdentity, RepositoryError> {
    let profile = files.profile().clone();
    let deal = deal_id.to_owned();
    let (input, batch) = tokio::task::spawn_blocking(move || {
        prepare_document(&deal, actor_id, document, chunks, file_bytes, profile)
    })
    .await
    .map_err(|_| RepositoryError::BlockingWorker("document preparation interrupted".into()))?
    .map_err(RepositoryError::invalid)?;
    files.persist_complete(input, batch, attempt).await
}
fn prepare_document(
    deal_id: &str,
    actor_id: i64,
    document: Document,
    mut chunks: Vec<DocumentChunk>,
    file_bytes: Vec<u8>,
    profile: EmbeddingProfile,
) -> Result<(FilePersistenceInput, PreparedChunks), String> {
    ensure_document_invariants(&document, &chunks)?;
    let mut input = build_file_persistence_input(deal_id, actor_id, &document, file_bytes)?;
    let mut rows = Vec::with_capacity(chunks.len());
    for chunk in &mut chunks {
        let chunk_index = i64::from(chunk.sequence_number);
        let (page_start, page_end) = page_range(chunk)?;
        rows.push((
            FileChunkResult {
                chunk_id: sha256_hex(
                    format!(
                        "{}\0{}\0{}\0{}\0{chunk_index}\0{}",
                        deal_id,
                        document.file_id,
                        input.version_id,
                        input.version_id,
                        chunk.content_hash
                    )
                    .as_bytes(),
                ),
                deal_id: deal_id.into(),
                file_id: document.file_id.clone(),
                version_id: input.version_id.clone(),
                index_generation: input.version_id.clone(),
                chunk_index,
                text: chunk.text.clone(),
                chunk_sha256: chunk.content_hash.clone(),
                token_count: i64::from(chunk.token_count),
                page_start,
                page_end,
                char_start: usize_to_i64(chunk.start_offset, "char_start")?,
                char_end: usize_to_i64(chunk.end_offset, "char_end")?,
                section_path: chunk.section_title.clone().unwrap_or_default(),
                created_at: input.timestamp.clone(),
            },
            chunk.embedding.take().ok_or("missing chunk embedding")?,
        ));
    }
    rows.sort_by_key(|(c, _)| c.chunk_index);
    input.manifest = Some(IdentityManifest {
        derivation_version: 2,
        document_id: document.document_id.clone(),
        active_generation: input.version_id.clone(),
        chunks: rows
            .iter()
            .map(|(c, _)| ChunkIdentity {
                chunk_id: c.chunk_id.clone(),
                index_generation: c.index_generation.clone(),
                chunk_index: c.chunk_index,
                chunk_sha256: c.chunk_sha256.clone(),
            })
            .collect(),
    });
    if infer_supported_image_mime_type(Path::new(&document.file_name)).is_some() {
        let mut accepted = chunks;
        for chunk in &mut accepted {
            chunk.embedding = None;
        }
        input.image_assembly_json = Some(
            serde_json::to_string(&crate::domains::documents::formats::image::ImageAssembly {
                document,
                chunks: accepted,
            })
            .map_err(|e| e.to_string())?,
        );
    }
    let batch = PreparedChunks::new(profile, rows).map_err(|e| e.to_string())?;
    Ok((input, batch))
}

pub(crate) fn build_file_persistence_input(
    deal_id: &str,
    actor_id: i64,
    document: &Document,
    file_bytes: Vec<u8>,
) -> Result<FilePersistenceInput, String> {
    let deal_id = normalized_nonempty("deal_id", deal_id)?;
    let file_id = normalized_nonempty("file_id", &document.file_id)?;
    if normalized_deal_identity(&document.deal_id)? != deal_id {
        return Err("document deal does not match upload context".into());
    }
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
        .map_err(|_| "file byte length does not fit in PostgreSQL INTEGER".to_string())?;

    let content_sha256 = sha256_hex(&file_bytes);
    if content_sha256 != document.content_hash {
        return Err(format!(
            "file bytes do not match content hash for document `{}`",
            document.document_id
        ));
    }
    let expected_document_id = document_id_from_content(&deal_id, &content_sha256);
    if expected_document_id != document.document_id {
        return Err(format!(
            "file bytes do not match document id `{}`",
            document.document_id
        ));
    }

    let mime_type = document_mime_type(Path::new(&display_name))
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
        actor_id,
        display_name,
        source_uri,
        metadata_json,
        timestamp,
        version_id,
        mime_type: mime_type.to_string(),
        content_sha256,
        byte_size,
        file_bytes,
        manifest: None,
        image_assembly_json: None,
    })
}

fn document_mime_type(path: &Path) -> Option<&'static str> {
    infer_supported_mime_type(path).or_else(|| infer_supported_image_mime_type(path))
}
fn ensure_document_invariants(document: &Document, chunks: &[DocumentChunk]) -> Result<(), String> {
    let mut chunk_indices = HashSet::with_capacity(chunks.len());
    let mut embedding_dimension = None;
    for chunk in chunks {
        ensure_chunk_belongs_to_document(document, chunk)?;
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

fn ensure_chunk_belongs_to_document(
    document: &Document,
    chunk: &DocumentChunk,
) -> Result<(), String> {
    if chunk.document_id != document.document_id {
        return Err(format!(
            "chunk `{}` belongs to document `{}`, not `{}`",
            chunk.chunk_id, chunk.document_id, document.document_id
        ));
    }
    if chunk.deal_id != document.deal_id {
        return Err(format!(
            "chunk `{}` belongs to user `{}`, not `{}`",
            chunk.chunk_id, chunk.deal_id, document.deal_id
        ));
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

fn normalized_deal_identity(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err("deal_id cannot be empty".to_string());
    }
    if normalized != value {
        return Err("deal_id must be trimmed".to_string());
    }
    Ok(normalized)
}

#[cfg(test)]
#[path = "../../../../tests/integration/domains/documents/ingestion/persistence_tests.rs"]
mod tests;
