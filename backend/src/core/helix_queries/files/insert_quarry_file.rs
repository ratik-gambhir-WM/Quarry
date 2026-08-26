use std::collections::HashSet;

use helix_db::dsl::prelude::*;
use helix_db::OnNodes;

use crate::core::nodes::document_node::{FileChunkNode, FileNode, FileVersionNode};

pub const QUARRY_FILE_LABEL: &str = "QuarryFile";
pub const FILE_VERSION_LABEL: &str = "FileVersion";
pub const FILE_CHUNK_LABEL: &str = "FileChunk";
pub const HAS_VERSION_LABEL: &str = "HAS_VERSION";
pub const CURRENT_VERSION_LABEL: &str = "CURRENT_VERSION";
pub const HAS_CHUNK_LABEL: &str = "HAS_CHUNK";

// Helix's /v1/query route uses Axum's default buffered-body limit of 2 MiB.
pub const HELIX_MAX_QUERY_BODY_BYTES: usize = 2_097_152;

pub fn insert_file_version_graph(
    file: FileNode,
    version: FileVersionNode,
    chunks: Vec<FileChunkNode>,
) -> Result<DynamicQueryRequest, String> {
    validate_graph_identity(&file, &version, &chunks)?;

    let chunk_params = chunks.iter().map(file_chunk_params).collect::<Vec<_>>();
    let query = insert_file_version_graph_route(
        file.workspace_id,
        file.file_id,
        file.display_name,
        version.version_id,
        version.mime_type,
        version.content_sha256,
        version.byte_size,
        version.index_generation,
        version.indexed_at,
        chunk_params,
    );
    validate_query_payload_size(&query, HELIX_MAX_QUERY_BODY_BYTES)?;
    Ok(query)
}

#[allow(clippy::too_many_arguments)]
#[register]
fn insert_file_version_graph_route(
    workspace_id: String,
    file_id: String,
    display_name: String,
    version_id: String,
    mime_type: String,
    content_sha256: String,
    byte_size: i64,
    index_generation: String,
    indexed_at: String,
    chunks: Vec<ParamObject>,
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
        &chunks,
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
        .var_as(
            "removed_version_chunks",
            g().n_with_label(FILE_CHUNK_LABEL)
                .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                .where_(Predicate::eq_param("file_id", "file_id"))
                .where_(Predicate::eq_param("version_id", "version_id"))
                .drop(),
        )
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
        .returning([
            "file",
            "version",
            "file_chunk",
            "removed_has_version",
            "added_has_version",
            "removed_current_version",
            "added_current_version",
            "removed_version_chunks",
            "version_has_chunk",
        ])
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
    query: &DynamicQueryRequest,
    max_payload_bytes: usize,
) -> Result<(), String> {
    let payload_bytes = query
        .to_json_bytes()
        .map_err(|error| format!("failed to serialize atomic Helix file graph query: {error}"))?
        .len();
    if payload_bytes > max_payload_bytes {
        return Err(format!(
            "atomic Helix file graph query is {payload_bytes} bytes, exceeding the configured {max_payload_bytes}-byte limit"
        ));
    }
    Ok(())
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

#[register]
pub fn create_document_indexes() -> WriteBatch {
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
            g().create_index_if_not_exists(IndexSpec::node_equality(FILE_VERSION_LABEL, "file_id")),
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
            g().create_index_if_not_exists(IndexSpec::node_equality(FILE_CHUNK_LABEL, "file_id")),
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
#[path = "../../../../tests/core/helix_queries/files/insert_quarry_file_tests.rs"]
mod tests;
