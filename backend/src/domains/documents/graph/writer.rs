use std::{collections::HashSet, num::NonZeroUsize};

use helix_db::dsl::prelude::*;
use helix_db::OnNodes;

use crate::domains::documents::graph::model::{FileChunkNode, FileNode, FileVersionNode};

pub const QUARRY_FILE_LABEL: &str = "QuarryFile";
pub const FILE_VERSION_LABEL: &str = "FileVersion";
pub const FILE_CHUNK_LABEL: &str = "FileChunk";
pub const HAS_VERSION_LABEL: &str = "HAS_VERSION";
pub const CURRENT_VERSION_LABEL: &str = "CURRENT_VERSION";
pub const HAS_CHUNK_LABEL: &str = "HAS_CHUNK";

pub const DOCUMENT_INDEX_NAMES: [&str; 13] = [
    "file_id_unique",
    "file_workspace_id",
    "file_display_name",
    "version_id_unique",
    "version_workspace_id",
    "version_file_id",
    "version_content_sha256",
    "chunk_id_unique",
    "chunk_workspace_id",
    "chunk_file_id",
    "chunk_version_id",
    "chunk_embedding",
    "chunk_text",
];

// Keep individual graph writes below the conservative 2 MiB preflight limit.
pub const HELIX_MAX_QUERY_BODY_BYTES: usize = 2_097_152;

#[derive(Debug)]
pub(crate) struct FileVersionGraphWrite {
    pub(crate) setup_query: QueryRequest,
    pub(crate) chunk_queries: Vec<QueryRequest>,
    pub(crate) publish_query: QueryRequest,
}

pub(crate) fn insert_file_version_graph(
    file: FileNode,
    version: FileVersionNode,
    chunks: Vec<FileChunkNode>,
) -> Result<FileVersionGraphWrite, String> {
    validate_graph_identity(&file, &version, &chunks)?;

    let setup_query = setup_file_version_graph_query(&file, &version)?;
    let chunk_queries = chunk_batch_queries(&file, &version, &chunks)?;
    let publish_query = publish_file_version_graph_query(&file, &version)?;
    Ok(FileVersionGraphWrite {
        setup_query,
        chunk_queries,
        publish_query,
    })
}

#[allow(clippy::too_many_arguments)]
#[query]
fn setup_file_version_graph_route(
    workspace_id: String,
    file_id: String,
    display_name: String,
    version_id: String,
    mime_type: String,
    content_sha256: String,
    byte_size: i64,
    index_generation: String,
    indexed_at: String,
) -> WriteBatch {
    let _ = (
        &workspace_id,
        &file_id,
        &display_name,
        &version_id,
        &mime_type,
        &content_sha256,
        &byte_size,
        &index_generation,
        &indexed_at,
    );

    write_batch()
        .var_as(
            "existing_file",
            g().n_with_label(QUARRY_FILE_LABEL)
                .where_(Predicate::eq_param("file_id", "file_id"))
                .where_(Predicate::eq_param("workspace_id", "workspace_id")),
        )
        .var_as_if(
            "created_file",
            BatchCondition::VarEmpty("existing_file".to_string()),
            g().add_n(
                QUARRY_FILE_LABEL,
                vec![
                    ("workspace_id", PropertyInput::param("workspace_id")),
                    ("file_id", PropertyInput::param("file_id")),
                    ("display_name", PropertyInput::param("display_name")),
                ],
            ),
        )
        .var_as(
            "canonical_file",
            g().n_with_label(QUARRY_FILE_LABEL)
                .where_(Predicate::eq_param("file_id", "file_id"))
                .where_(Predicate::eq_param("workspace_id", "workspace_id")),
        )
        .var_as(
            "file",
            g().n(NodeRef::var("canonical_file"))
                .set_property("display_name", PropertyInput::param("display_name"))
                .project(file_projection()),
        )
        .var_as("existing_version", version_by_immutable_identity())
        .var_as_if(
            "created_version",
            BatchCondition::VarEmpty("existing_version".to_string()),
            g().add_n(
                FILE_VERSION_LABEL,
                vec![
                    ("workspace_id", PropertyInput::param("workspace_id")),
                    ("file_id", PropertyInput::param("file_id")),
                    ("version_id", PropertyInput::param("version_id")),
                    ("mime_type", PropertyInput::param("mime_type")),
                    ("content_sha256", PropertyInput::param("content_sha256")),
                    ("byte_size", PropertyInput::param("byte_size")),
                    ("index_generation", PropertyInput::param("index_generation")),
                    ("indexed_at", PropertyInput::param("indexed_at")),
                ],
            ),
        )
        .var_as("canonical_version", version_by_immutable_identity())
        .var_as(
            "version",
            g().n(NodeRef::var("canonical_version"))
                .set_property("index_generation", PropertyInput::param("index_generation"))
                .set_property("indexed_at", PropertyInput::param("indexed_at"))
                .project(file_version_projection()),
        )
        .var_as(
            "removed_version_chunks",
            g().n_with_label(FILE_CHUNK_LABEL)
                .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                .where_(Predicate::eq_param("file_id", "file_id"))
                .where_(Predicate::eq_param("version_id", "version_id"))
                .drop(),
        )
        .returning(["file", "version", "removed_version_chunks"])
}

#[query]
fn publish_file_version_graph_route(
    workspace_id: String,
    file_id: String,
    version_id: String,
) -> WriteBatch {
    let _ = (&workspace_id, &file_id, &version_id);

    write_batch()
        .var_as(
            "canonical_file",
            g().n_with_label(QUARRY_FILE_LABEL)
                .where_(Predicate::eq_param("file_id", "file_id"))
                .where_(Predicate::eq_param("workspace_id", "workspace_id")),
        )
        .var_as(
            "canonical_version",
            g().n_with_label(FILE_VERSION_LABEL)
                .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                .where_(Predicate::eq_param("file_id", "file_id"))
                .where_(Predicate::eq_param("version_id", "version_id")),
        )
        .var_as(
            "removed_has_version",
            g().n(NodeRef::var("canonical_file"))
                .drop_edge_labeled(NodeRef::var("canonical_version"), HAS_VERSION_LABEL),
        )
        .var_as(
            "added_has_version",
            g().n(NodeRef::var("canonical_file")).add_e(
                HAS_VERSION_LABEL,
                NodeRef::var("canonical_version"),
                Vec::<(&str, PropertyInput)>::new(),
            ),
        )
        .var_as(
            "old_current_versions",
            g().n(NodeRef::var("canonical_file"))
                .out(Some(CURRENT_VERSION_LABEL)),
        )
        .var_as(
            "removed_current_version",
            g().n(NodeRef::var("canonical_file"))
                .drop_edge_labeled(NodeRef::var("old_current_versions"), CURRENT_VERSION_LABEL),
        )
        .var_as(
            "added_current_version",
            g().n(NodeRef::var("canonical_file")).add_e(
                CURRENT_VERSION_LABEL,
                NodeRef::var("canonical_version"),
                Vec::<(&str, PropertyInput)>::new(),
            ),
        )
        .returning([
            "removed_has_version",
            "added_has_version",
            "removed_current_version",
            "added_current_version",
        ])
}

#[query]
fn insert_file_version_chunk_batch_route(
    workspace_id: String,
    file_id: String,
    version_id: String,
    chunks: Vec<ParamObject>,
) -> WriteBatch {
    let _ = (&workspace_id, &file_id, &version_id, &chunks);

    write_batch()
        .for_each_param(
            "chunks",
            write_batch()
                .var_as(
                    "chunk_version",
                    g().n_with_label(FILE_VERSION_LABEL)
                        .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                        .where_(Predicate::eq_param("file_id", "file_id"))
                        .where_(Predicate::eq_param("version_id", "version_id")),
                )
                .var_as_if(
                    "chunk",
                    BatchCondition::VarNotEmpty("chunk_version".to_string()),
                    g().add_n(
                        FILE_CHUNK_LABEL,
                        vec![
                            ("chunk_id", PropertyInput::param("chunk_id")),
                            ("workspace_id", PropertyInput::param("workspace_id")),
                            ("file_id", PropertyInput::param("file_id")),
                            ("version_id", PropertyInput::param("version_id")),
                            ("index_generation", PropertyInput::param("index_generation")),
                            ("chunk_index", PropertyInput::param("chunk_index")),
                            ("text", PropertyInput::param("text")),
                            ("embedding", PropertyInput::param("embedding")),
                            ("chunk_sha256", PropertyInput::param("chunk_sha256")),
                            ("token_count", PropertyInput::param("token_count")),
                            ("page_start", PropertyInput::param("page_start")),
                            ("page_end", PropertyInput::param("page_end")),
                            ("char_start", PropertyInput::param("char_start")),
                            ("char_end", PropertyInput::param("char_end")),
                            ("section_path", PropertyInput::param("section_path")),
                            ("created_at", PropertyInput::param("created_at")),
                        ],
                    ),
                )
                .var_as_if(
                    "file_chunk",
                    BatchCondition::VarNotEmpty("chunk".to_string()),
                    g().n(NodeRef::var("chunk"))
                        .project(file_chunk_projection()),
                )
                .var_as_if(
                    "version_has_chunk",
                    BatchCondition::VarNotEmpty("chunk".to_string()),
                    g().n(NodeRef::var("chunk_version")).add_e(
                        HAS_CHUNK_LABEL,
                        NodeRef::var("chunk"),
                        Vec::<(&str, PropertyInput)>::new(),
                    ),
                ),
        )
        .returning(["file_chunk", "version_has_chunk"])
}

fn setup_file_version_graph_query(
    file: &FileNode,
    version: &FileVersionNode,
) -> Result<QueryRequest, String> {
    let query = setup_file_version_graph_route(
        file.workspace_id.clone(),
        file.file_id.clone(),
        file.display_name.clone(),
        version.version_id.clone(),
        version.mime_type.clone(),
        version.content_sha256.clone(),
        version.byte_size,
        version.index_generation.clone(),
        version.indexed_at.clone(),
    )
    .map_err(|error| format!("failed to construct Helix file-version setup query: {error}"))?;
    validate_query_payload_size(
        &query,
        HELIX_MAX_QUERY_BODY_BYTES,
        "Helix file-version setup query",
    )?;
    Ok(query)
}

fn publish_file_version_graph_query(
    file: &FileNode,
    version: &FileVersionNode,
) -> Result<QueryRequest, String> {
    let query = publish_file_version_graph_route(
        file.workspace_id.clone(),
        file.file_id.clone(),
        version.version_id.clone(),
    )
    .map_err(|error| format!("failed to construct Helix file-version publish query: {error}"))?;
    validate_query_payload_size(
        &query,
        HELIX_MAX_QUERY_BODY_BYTES,
        "Helix file-version publish query",
    )?;
    Ok(query)
}

fn chunk_batch_queries(
    file: &FileNode,
    version: &FileVersionNode,
    chunks: &[FileChunkNode],
) -> Result<Vec<QueryRequest>, String> {
    let empty_query = insert_file_version_chunk_batch_query(file, version, Vec::new())?;
    let empty_payload_bytes = serialized_query_payload_bytes(&empty_query)?;
    let mut queries = Vec::new();
    let mut batch = Vec::new();
    let mut batch_payload_bytes = empty_payload_bytes;

    for chunk in chunks {
        let chunk_param = file_chunk_params(chunk);
        let chunk_payload_bytes = serialized_chunk_param_contribution_bytes(
            file,
            version,
            &chunk_param,
            empty_payload_bytes,
        )?;
        let candidate_payload_bytes = next_chunk_batch_payload_bytes(
            batch_payload_bytes,
            !batch.is_empty(),
            chunk_payload_bytes,
        )?;

        if candidate_payload_bytes <= HELIX_MAX_QUERY_BODY_BYTES {
            batch.push(chunk_param);
            batch_payload_bytes = candidate_payload_bytes;
            continue;
        }

        if batch.is_empty() {
            let query = insert_file_version_chunk_batch_query(file, version, vec![chunk_param])?;
            return Err(oversized_query_error(
                &query,
                HELIX_MAX_QUERY_BODY_BYTES,
                "Helix file chunk batch query",
            )?);
        }

        let query =
            insert_file_version_chunk_batch_query(file, version, std::mem::take(&mut batch))?;
        validate_query_payload_size(
            &query,
            HELIX_MAX_QUERY_BODY_BYTES,
            "Helix file chunk batch query",
        )?;
        queries.push(query);

        batch_payload_bytes =
            next_chunk_batch_payload_bytes(empty_payload_bytes, false, chunk_payload_bytes)?;
        batch.push(chunk_param);
    }

    if !batch.is_empty() {
        let query = insert_file_version_chunk_batch_query(file, version, batch)?;
        validate_query_payload_size(
            &query,
            HELIX_MAX_QUERY_BODY_BYTES,
            "Helix file chunk batch query",
        )?;
        queries.push(query);
    }

    Ok(queries)
}

fn insert_file_version_chunk_batch_query(
    file: &FileNode,
    version: &FileVersionNode,
    chunks: Vec<ParamObject>,
) -> Result<QueryRequest, String> {
    insert_file_version_chunk_batch_route(
        file.workspace_id.clone(),
        file.file_id.clone(),
        version.version_id.clone(),
        chunks,
    )
    .map_err(|error| format!("failed to construct Helix file chunk batch query: {error}"))
}

fn serialized_chunk_param_contribution_bytes(
    file: &FileNode,
    version: &FileVersionNode,
    chunk: &ParamObject,
    empty_payload_bytes: usize,
) -> Result<usize, String> {
    let query = insert_file_version_chunk_batch_query(file, version, vec![chunk.clone()])?;
    serialized_query_payload_bytes(&query)?
        .checked_sub(empty_payload_bytes)
        .ok_or_else(|| "Helix file chunk parameter serialization underflowed usize".to_string())
}

fn next_chunk_batch_payload_bytes(
    current_payload_bytes: usize,
    has_existing_chunks: bool,
    chunk_payload_bytes: usize,
) -> Result<usize, String> {
    current_payload_bytes
        .checked_add(usize::from(has_existing_chunks))
        .and_then(|bytes| bytes.checked_add(chunk_payload_bytes))
        .ok_or_else(|| "Helix file chunk batch payload size overflowed usize".to_string())
}

fn version_by_immutable_identity() -> Traversal<OnNodes> {
    g().n_with_label(FILE_VERSION_LABEL)
        .where_(Predicate::eq_param("workspace_id", "workspace_id"))
        .where_(Predicate::eq_param("file_id", "file_id"))
        .where_(Predicate::eq_param("version_id", "version_id"))
        .where_(Predicate::eq_param("mime_type", "mime_type"))
        .where_(Predicate::eq_param("content_sha256", "content_sha256"))
        .where_(Predicate::eq_param("byte_size", "byte_size"))
}

fn validate_graph_identity(
    file: &FileNode,
    version: &FileVersionNode,
    chunks: &[FileChunkNode],
) -> Result<(), String> {
    for (field, value) in [
        ("workspace_id", file.workspace_id.as_str()),
        ("file_id", file.file_id.as_str()),
        ("display_name", file.display_name.as_str()),
        ("version_id", version.version_id.as_str()),
        ("mime_type", version.mime_type.as_str()),
        ("content_sha256", version.content_sha256.as_str()),
        ("index_generation", version.index_generation.as_str()),
        ("indexed_at", version.indexed_at.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{field} cannot be empty"));
        }
    }
    if version.byte_size < 0 {
        return Err("byte_size cannot be negative".to_string());
    }
    if file.workspace_id != version.workspace_id || file.file_id != version.file_id {
        return Err("file and version graph identities do not match".to_string());
    }
    let mut chunk_ids = HashSet::with_capacity(chunks.len());
    let mut chunk_indices = HashSet::with_capacity(chunks.len());
    let mut embedding_dimension = None;
    for chunk in chunks {
        if chunk.workspace_id != version.workspace_id
            || chunk.file_id != version.file_id
            || chunk.version_id != version.version_id
            || chunk.index_generation != version.index_generation
        {
            return Err(format!(
                "chunk `{}` graph identity does not match its file version",
                chunk.chunk_id
            ));
        }
        if chunk.chunk_id.trim().is_empty()
            || chunk.chunk_sha256.trim().is_empty()
            || chunk.created_at.trim().is_empty()
        {
            return Err("chunk identity and timestamp fields cannot be empty".to_string());
        }
        if !chunk_ids.insert(&chunk.chunk_id) || !chunk_indices.insert(chunk.chunk_index) {
            return Err(format!(
                "chunk `{}` duplicates a graph chunk identity or index",
                chunk.chunk_id
            ));
        }
        if chunk.chunk_index < 0
            || chunk.token_count < 0
            || chunk.char_start < 0
            || chunk.char_end < chunk.char_start
        {
            return Err(format!(
                "chunk `{}` has an invalid numeric range",
                chunk.chunk_id
            ));
        }
        if chunk.page_start.is_some() != chunk.page_end.is_some()
            || chunk
                .page_start
                .zip(chunk.page_end)
                .is_some_and(|(start, end)| start > end)
        {
            return Err(format!(
                "chunk `{}` has an invalid page range",
                chunk.chunk_id
            ));
        }
        if chunk.embedding.is_empty() || chunk.embedding.iter().any(|value| !value.is_finite()) {
            return Err(format!(
                "chunk `{}` has an invalid embedding",
                chunk.chunk_id
            ));
        }
        match embedding_dimension {
            Some(expected) if expected != chunk.embedding.len() => {
                return Err(format!(
                    "chunk `{}` embedding dimension does not match the version",
                    chunk.chunk_id
                ));
            }
            None => embedding_dimension = Some(chunk.embedding.len()),
            _ => {}
        }
    }
    Ok(())
}

fn validate_query_payload_size(
    query: &QueryRequest,
    max_payload_bytes: usize,
    query_description: &str,
) -> Result<(), String> {
    let payload_bytes = serialized_query_payload_bytes(query)?;
    if payload_bytes > max_payload_bytes {
        return Err(payload_limit_error(
            payload_bytes,
            max_payload_bytes,
            query_description,
        ));
    }
    Ok(())
}

fn serialized_query_payload_bytes(query: &QueryRequest) -> Result<usize, String> {
    query
        .to_json_bytes()
        .map_err(|error| format!("failed to serialize Helix graph query: {error}"))
        .map(|payload| payload.len())
}

fn oversized_query_error(
    query: &QueryRequest,
    max_payload_bytes: usize,
    query_description: &str,
) -> Result<String, String> {
    let payload_bytes = serialized_query_payload_bytes(query)?;
    Ok(payload_limit_error(
        payload_bytes,
        max_payload_bytes,
        query_description,
    ))
}

fn payload_limit_error(
    payload_bytes: usize,
    max_payload_bytes: usize,
    query_description: &str,
) -> String {
    format!(
        "{query_description} is {payload_bytes} bytes, exceeding the configured {max_payload_bytes}-byte limit"
    )
}

fn file_chunk_params(chunk: &FileChunkNode) -> ParamObject {
    ParamObject::from([
        (
            "chunk_id".to_string(),
            PropertyValue::String(chunk.chunk_id.clone()),
        ),
        (
            "workspace_id".to_string(),
            PropertyValue::String(chunk.workspace_id.clone()),
        ),
        (
            "file_id".to_string(),
            PropertyValue::String(chunk.file_id.clone()),
        ),
        (
            "version_id".to_string(),
            PropertyValue::String(chunk.version_id.clone()),
        ),
        (
            "index_generation".to_string(),
            PropertyValue::String(chunk.index_generation.clone()),
        ),
        (
            "chunk_index".to_string(),
            PropertyValue::I64(chunk.chunk_index),
        ),
        (
            "text".to_string(),
            PropertyValue::String(chunk.text.clone()),
        ),
        (
            "embedding".to_string(),
            PropertyValue::F32Array(chunk.embedding.clone()),
        ),
        (
            "chunk_sha256".to_string(),
            PropertyValue::String(chunk.chunk_sha256.clone()),
        ),
        (
            "token_count".to_string(),
            PropertyValue::I64(chunk.token_count),
        ),
        (
            "page_start".to_string(),
            chunk
                .page_start
                .map_or(PropertyValue::Null, PropertyValue::I64),
        ),
        (
            "page_end".to_string(),
            chunk
                .page_end
                .map_or(PropertyValue::Null, PropertyValue::I64),
        ),
        (
            "char_start".to_string(),
            PropertyValue::I64(chunk.char_start),
        ),
        ("char_end".to_string(), PropertyValue::I64(chunk.char_end)),
        (
            "section_path".to_string(),
            PropertyValue::String(chunk.section_path.clone()),
        ),
        (
            "created_at".to_string(),
            PropertyValue::String(chunk.created_at.clone()),
        ),
    ])
}

pub fn create_document_indexes(vector_dimension: NonZeroUsize) -> QueryRequest {
    QueryRequest::write(
        write_batch()
            .var_as(
                "file_id_unique",
                g().create_index_if_not_exists(IndexSpec::node_unique_equality(
                    QUARRY_FILE_LABEL,
                    "file_id",
                )),
            )
            .var_as(
                "file_workspace_id",
                g().create_index_if_not_exists(IndexSpec::node_equality(
                    QUARRY_FILE_LABEL,
                    "workspace_id",
                )),
            )
            .var_as(
                "file_display_name",
                g().create_index_if_not_exists(IndexSpec::node_text(
                    QUARRY_FILE_LABEL,
                    "display_name",
                    None::<&str>,
                )),
            )
            .var_as(
                "version_id_unique",
                g().create_index_if_not_exists(IndexSpec::node_unique_equality(
                    FILE_VERSION_LABEL,
                    "version_id",
                )),
            )
            .var_as(
                "version_workspace_id",
                g().create_index_if_not_exists(IndexSpec::node_equality(
                    FILE_VERSION_LABEL,
                    "workspace_id",
                )),
            )
            .var_as(
                "version_file_id",
                g().create_index_if_not_exists(IndexSpec::node_equality(
                    FILE_VERSION_LABEL,
                    "file_id",
                )),
            )
            .var_as(
                "version_content_sha256",
                g().create_index_if_not_exists(IndexSpec::node_equality(
                    FILE_VERSION_LABEL,
                    "content_sha256",
                )),
            )
            .var_as(
                "chunk_id_unique",
                g().create_index_if_not_exists(IndexSpec::node_unique_equality(
                    FILE_CHUNK_LABEL,
                    "chunk_id",
                )),
            )
            .var_as(
                "chunk_workspace_id",
                g().create_index_if_not_exists(IndexSpec::node_equality(
                    FILE_CHUNK_LABEL,
                    "workspace_id",
                )),
            )
            .var_as(
                "chunk_file_id",
                g().create_index_if_not_exists(IndexSpec::node_equality(
                    FILE_CHUNK_LABEL,
                    "file_id",
                )),
            )
            .var_as(
                "chunk_version_id",
                g().create_index_if_not_exists(IndexSpec::node_equality(
                    FILE_CHUNK_LABEL,
                    "version_id",
                )),
            )
            .var_as(
                "chunk_embedding",
                g().create_index_if_not_exists(IndexSpec::node_vector(
                    FILE_CHUNK_LABEL,
                    "embedding",
                    vector_dimension,
                    VectorDistanceMetric::Cosine,
                    Some("workspace_id"),
                )),
            )
            .var_as(
                "chunk_text",
                g().create_index_if_not_exists(IndexSpec::node_text(
                    FILE_CHUNK_LABEL,
                    "text",
                    Some("workspace_id"),
                )),
            )
            .returning([
                "file_id_unique",
                "file_workspace_id",
                "file_display_name",
                "version_id_unique",
                "version_workspace_id",
                "version_file_id",
                "version_content_sha256",
                "chunk_id_unique",
                "chunk_workspace_id",
                "chunk_file_id",
                "chunk_version_id",
                "chunk_embedding",
                "chunk_text",
            ]),
    )
}

pub fn get_document_index_operation(operation_id: String) -> QueryRequest {
    QueryRequest::read(
        read_batch()
            .var_as("status", g().get_index_operation(operation_id))
            .returning(["status"]),
    )
}

pub(super) fn file_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::new("workspace_id"),
        PropertyProjection::new("file_id"),
        PropertyProjection::new("display_name"),
    ]
}

pub(super) fn file_version_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::new("workspace_id"),
        PropertyProjection::new("file_id"),
        PropertyProjection::new("version_id"),
        PropertyProjection::new("mime_type"),
        PropertyProjection::new("content_sha256"),
        PropertyProjection::new("byte_size"),
        PropertyProjection::new("index_generation"),
        PropertyProjection::new("indexed_at"),
    ]
}

pub(super) fn file_chunk_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::new("chunk_id"),
        PropertyProjection::new("workspace_id"),
        PropertyProjection::new("file_id"),
        PropertyProjection::new("version_id"),
        PropertyProjection::new("index_generation"),
        PropertyProjection::new("chunk_index"),
        PropertyProjection::new("text"),
        PropertyProjection::new("chunk_sha256"),
        PropertyProjection::new("token_count"),
        PropertyProjection::new("page_start"),
        PropertyProjection::new("page_end"),
        PropertyProjection::new("char_start"),
        PropertyProjection::new("char_end"),
        PropertyProjection::new("section_path"),
        PropertyProjection::new("created_at"),
    ]
}

#[cfg(test)]
#[path = "../../../../tests/integration/domains/documents/graph/writer_tests.rs"]
mod tests;
