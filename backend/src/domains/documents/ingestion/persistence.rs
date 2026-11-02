use crate::domains::documents::{
    graph::{
        model::{FileChunkNode, FileNode, FileVersionNode},
        repository::{DocumentGraphWriter, PreparedDocumentGraph},
    },
    model::{Document, DocumentChunk},
    store::{
        model::{FilePersistenceInput, PersistedFileIdentity},
        sqlite::DocumentStore,
    },
};
use crate::shared::file_policy::{infer_supported_image_mime_type, infer_supported_mime_type};
use crate::shared::ids::{document_id_from_content, file_version_id, sha256_hex};
use chrono::{SecondsFormat, Utc};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug)]
pub struct DocumentPersistenceResult {
    pub insert_blob_result: PersistedFileIdentity,
    pub insert_document_chunk_result: Value,
}

/// Immutable SQLite input and graph request prepared before canonical
/// persistence. The graph request is built once, with one timestamp, and has
/// already passed the writer's exact serialized-size preflight.
struct PreparedDocumentPersistence {
    expected_identity: PersistedFileIdentity,
    file_persistence: FilePersistenceInput,
    graph: PreparedDocumentGraph,
}

pub(crate) async fn persist_document_and_chunks(
    files: &DocumentStore,
    graph_writer: &DocumentGraphWriter,
    deal_id: &str,
    actor_id: i64,
    document: Document,
    chunks: Vec<DocumentChunk>,
    file_bytes: Vec<u8>,
) -> Result<DocumentPersistenceResult, String> {
    let mut prepared = prepare_document_persistence(
        graph_writer,
        deal_id,
        actor_id,
        &document,
        &chunks,
        file_bytes,
    )?;
    if let Some(manifest) = files
        .manifest(prepared.expected_identity.version_id.clone())
        .await
        .map_err(|e| e.to_string())?
    {
        // Legacy versions keep exact IDs. A changed parser must never invent replacements.
        let mut nodes = prepared.graph.chunk_nodes.clone();
        let active = manifest
            .chunks
            .iter()
            .filter(|c| c.index_generation == manifest.active_generation)
            .collect::<Vec<_>>();
        if nodes.len() != active.len() {
            return Err("parser output differs from immutable identity manifest; maintenance recovery required".into());
        }
        for node in &mut nodes {
            let identity = active
                .iter()
                .find(|c| c.chunk_index == node.chunk_index && c.chunk_sha256 == node.chunk_sha256)
                .ok_or("parser chunk differs from immutable identity manifest")?;
            node.chunk_id.clone_from(&identity.chunk_id);
            node.index_generation.clone_from(&identity.index_generation);
        }
        let mut version = prepared.graph.version_node.clone();
        version
            .index_generation
            .clone_from(&manifest.active_generation);
        prepared.graph = graph_writer
            .preflight_graph(prepared.graph.file_node.clone(), version, nodes)
            .map_err(|e| e.to_string())?;
        let mut metadata: Value = serde_json::from_str(&prepared.file_persistence.metadata_json)
            .map_err(|e| e.to_string())?;
        metadata["documentId"] = Value::String(manifest.document_id.clone());
        prepared.file_persistence.metadata_json =
            serde_json::to_string(&metadata).map_err(|e| e.to_string())?;
        prepared.file_persistence.manifest = Some(manifest);
    }
    persist_prepared_document_and_chunks(files, graph_writer, prepared).await
}

fn prepare_document_persistence(
    graph_writer: &DocumentGraphWriter,
    deal_id: &str,
    actor_id: i64,
    document: &Document,
    chunks: &[DocumentChunk],
    file_bytes: Vec<u8>,
) -> Result<PreparedDocumentPersistence, String> {
    ensure_document_graph_invariants(document, chunks)?;
    let mut file_persistence =
        build_file_persistence_input(deal_id, actor_id, document, file_bytes)?;
    if infer_supported_image_mime_type(Path::new(&document.file_name)).is_some() {
        let mut chunks = chunks.to_vec();
        for chunk in &mut chunks {
            chunk.embedding = None;
        }
        file_persistence.image_assembly_json = Some(
            serde_json::to_string(&crate::domains::documents::formats::image::ImageAssembly {
                document: document.clone(),
                chunks,
            })
            .map_err(|e| e.to_string())?,
        );
    }
    let expected_identity = PersistedFileIdentity {
        file_id: file_persistence.file_id.clone(),
        deal_id: file_persistence.deal_id.clone(),
        display_name: file_persistence.display_name.clone(),
        version_id: file_persistence.version_id.clone(),
    };
    let (file_node, version_node, chunk_nodes) = build_helix_graph_nodes_at(
        &expected_identity,
        document,
        chunks,
        &file_persistence.timestamp,
    )?;
    file_persistence.manifest = Some(
        crate::domains::documents::store::identity::IdentityManifest {
            derivation_version: 2,
            document_id: document.document_id.clone(),
            active_generation: version_node.index_generation.clone(),
            chunks: chunk_nodes
                .iter()
                .map(
                    |c| crate::domains::documents::store::identity::ChunkIdentity {
                        chunk_id: c.chunk_id.clone(),
                        index_generation: c.index_generation.clone(),
                        chunk_index: c.chunk_index,
                        chunk_sha256: c.chunk_sha256.clone(),
                    },
                )
                .collect(),
        },
    );
    let graph = graph_writer
        .preflight_graph(file_node, version_node, chunk_nodes)
        .map_err(|error| error.to_string())?;
    Ok(PreparedDocumentPersistence {
        expected_identity,
        file_persistence,
        graph,
    })
}

async fn persist_prepared_document_and_chunks(
    files: &DocumentStore,
    graph_writer: &DocumentGraphWriter,
    prepared: PreparedDocumentPersistence,
) -> Result<DocumentPersistenceResult, String> {
    let insert_blob_result = files
        .persist(prepared.file_persistence)
        .await
        .map_err(|error| error.to_string())?;
    if insert_blob_result != prepared.expected_identity {
        return Err(
            "committed SQLite identity does not match the prepared ingestion plan".to_string(),
        );
    }
    if !prepared.graph.matches_identity(
        &insert_blob_result.deal_id,
        &insert_blob_result.file_id,
        &insert_blob_result.display_name,
        &insert_blob_result.version_id,
    ) {
        return Err(
            "prepared Helix graph identity does not match the committed SQLite identity"
                .to_string(),
        );
    }
    let insert_document_chunk_result = graph_writer
        .insert_prepared_graph(prepared.graph)
        .await
        .map_err(|error| error.to_string())
    .map_err(|error| {
        format!(
            "Helix indexing failed after SQLite committed file_id `{}` and version_id `{}`: {error}",
            insert_blob_result.file_id, insert_blob_result.version_id
        )
    })?;

    Ok(DocumentPersistenceResult {
        insert_blob_result,
        insert_document_chunk_result,
    })
}

// These are persistence invariants over parser-derived data, not HTTP request validation.
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
        .map_err(|_| "file byte length does not fit in SQLite INTEGER".to_string())?;

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

#[cfg(test)]
fn build_helix_graph_nodes(
    insert_blob_result: &PersistedFileIdentity,
    document: &Document,
    chunks: &[DocumentChunk],
) -> Result<(FileNode, FileVersionNode, Vec<FileChunkNode>), String> {
    build_helix_graph_nodes_at(
        insert_blob_result,
        document,
        chunks,
        &Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    )
}

fn build_helix_graph_nodes_at(
    insert_blob_result: &PersistedFileIdentity,
    document: &Document,
    chunks: &[DocumentChunk],
    indexed_at: &str,
) -> Result<(FileNode, FileVersionNode, Vec<FileChunkNode>), String> {
    ensure_document_graph_invariants(document, chunks)?;
    if insert_blob_result.deal_id != document.deal_id
        || insert_blob_result.file_id != document.file_id
        || insert_blob_result.display_name != document.file_name
    {
        return Err(
            "committed SQLite file identity does not match the ingestion document".to_string(),
        );
    }
    let expected_version_id = file_version_id(&document.file_id, &document.content_hash);
    if insert_blob_result.version_id != expected_version_id {
        return Err(
            "committed SQLite version identity does not match the ingestion document".to_string(),
        );
    }

    let file_node = build_file_node(insert_blob_result);
    let version_node = build_file_version_node_at(insert_blob_result, document, indexed_at)?;
    let chunk_nodes = chunks
        .iter()
        .map(|chunk| build_file_chunk_node(insert_blob_result, &version_node, chunk))
        .collect::<Result<Vec<_>, String>>()?;
    Ok((file_node, version_node, chunk_nodes))
}

fn build_file_node(insert_blob_result: &PersistedFileIdentity) -> FileNode {
    FileNode {
        deal_id: insert_blob_result.deal_id.clone(),
        file_id: insert_blob_result.file_id.clone(),
        display_name: insert_blob_result.display_name.clone(),
    }
}

#[cfg(test)]
fn build_file_version_node(
    insert_blob_result: &PersistedFileIdentity,
    document: &Document,
) -> Result<FileVersionNode, String> {
    build_file_version_node_at(
        insert_blob_result,
        document,
        &Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    )
}

fn build_file_version_node_at(
    insert_blob_result: &PersistedFileIdentity,
    document: &Document,
    indexed_at: &str,
) -> Result<FileVersionNode, String> {
    let mime_type = document_mime_type(Path::new(&document.file_name))
        .ok_or_else(|| format!("unsupported file type for `{}`", document.file_name))?
        .to_string();
    let byte_size = i64::try_from(document.file_size_bytes)
        .map_err(|_| "document byte size does not fit in i64".to_string())?;

    Ok(FileVersionNode {
        deal_id: insert_blob_result.deal_id.clone(),
        file_id: insert_blob_result.file_id.clone(),
        version_id: insert_blob_result.version_id.clone(),
        mime_type,
        content_sha256: document.content_hash.clone(),
        byte_size,
        index_generation: insert_blob_result.version_id.clone(),
        indexed_at: indexed_at.to_string(),
    })
}

fn document_mime_type(path: &Path) -> Option<&'static str> {
    infer_supported_mime_type(path).or_else(|| infer_supported_image_mime_type(path))
}

fn build_file_chunk_node(
    insert_blob_result: &PersistedFileIdentity,
    version_node: &FileVersionNode,
    chunk: &DocumentChunk,
) -> Result<FileChunkNode, String> {
    let chunk_index = i64::from(chunk.sequence_number);
    let (page_start, page_end) = page_range(chunk)?;

    Ok(FileChunkNode {
        chunk_id: deterministic_file_chunk_id(
            &insert_blob_result.deal_id,
            &insert_blob_result.file_id,
            &insert_blob_result.version_id,
            &version_node.index_generation,
            chunk_index,
            &chunk.content_hash,
        ),
        deal_id: insert_blob_result.deal_id.clone(),
        file_id: insert_blob_result.file_id.clone(),
        version_id: insert_blob_result.version_id.clone(),
        index_generation: version_node.index_generation.clone(),
        chunk_index,
        text: chunk.text.clone(),
        embedding: chunk
            .embedding
            .clone()
            .ok_or_else(|| format!("chunk `{}` does not contain an embedding", chunk.chunk_id))?,
        chunk_sha256: chunk.content_hash.clone(),
        token_count: i64::from(chunk.token_count),
        page_start,
        page_end,
        char_start: usize_to_i64(chunk.start_offset, "char_start")?,
        char_end: usize_to_i64(chunk.end_offset, "char_end")?,
        section_path: chunk.section_title.clone().unwrap_or_default(),
        created_at: version_node.indexed_at.clone(),
    })
}

fn deterministic_file_chunk_id(
    deal_id: &str,
    file_id: &str,
    version_id: &str,
    index_generation: &str,
    chunk_index: i64,
    chunk_sha256: &str,
) -> String {
    sha256_hex(
        format!(
            "{deal_id}\0{file_id}\0{version_id}\0{index_generation}\0{chunk_index}\0{chunk_sha256}"
        )
        .as_bytes(),
    )
}

fn ensure_document_graph_invariants(
    document: &Document,
    chunks: &[DocumentChunk],
) -> Result<(), String> {
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
