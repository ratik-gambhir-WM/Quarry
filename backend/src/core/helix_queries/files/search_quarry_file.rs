use helix_db::dsl::prelude::*;
use helix_db::OnNodes;
use serde::{Deserialize, Serialize};

use crate::core::nodes::document_node::{FileNode, FileVersionNode};

use super::insert_quarry_file::{
    file_chunk_projection, file_projection, file_version_projection, CURRENT_VERSION_LABEL,
    FILE_CHUNK_LABEL, FILE_VERSION_LABEL, HAS_CHUNK_LABEL, HAS_VERSION_LABEL, QUARRY_FILE_LABEL,
};

pub const MAX_FILE_CHUNK_SEARCH_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChunkVectorSearch {
    pub workspace_id: String,
    pub query_embedding: Vec<f32>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChunkKeywordSearch {
    pub workspace_id: String,
    pub query_text: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelixDocumentVersion {
    pub file: FileNode,
    pub version: FileVersionNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChunkResult {
    pub chunk_id: String,
    pub workspace_id: String,
    pub file_id: String,
    pub version_id: String,
    pub index_generation: String,
    pub chunk_index: i64,
    pub text: String,
    pub chunk_sha256: String,
    pub token_count: i64,
    pub page_start: Option<i64>,
    pub page_end: Option<i64>,
    pub char_start: i64,
    pub char_end: i64,
    pub section_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorFileChunkHit {
    #[serde(flatten)]
    pub chunk: FileChunkResult,
    pub distance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordFileChunkHit {
    #[serde(flatten)]
    pub chunk: FileChunkResult,
    pub score: f64,
}

pub fn find_current_helix_document_by_content_hash(
    workspace_id: String,
    content_sha256: String,
) -> Result<DynamicQueryRequest, String> {
    validate_nonempty("workspace_id", &workspace_id)?;
    validate_nonempty("content_sha256", &content_sha256)?;
    Ok(find_current_helix_document_by_content_hash_route(
        workspace_id,
        content_sha256,
    ))
}

#[register]
fn find_current_helix_document_by_content_hash_route(
    workspace_id: String,
    content_sha256: String,
) -> ReadBatch {
    let _ = (&workspace_id, &content_sha256);
    read_batch()
        .var_as(
            "candidate_versions",
            g().n_with_label(FILE_VERSION_LABEL)
                .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                .where_(Predicate::eq_param("content_sha256", "content_sha256")),
        )
        .var_as(
            "canonical_file",
            g().n(NodeRef::var("candidate_versions"))
                .in_(Some(CURRENT_VERSION_LABEL))
                .where_(Predicate::eq_param("workspace_id", "workspace_id")),
        )
        .var_as(
            "canonical_version",
            g().n(NodeRef::var("canonical_file"))
                .out(Some(CURRENT_VERSION_LABEL))
                .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                .where_(Predicate::eq_param("content_sha256", "content_sha256")),
        )
        .var_as(
            "file",
            g().n(NodeRef::var("canonical_file"))
                .project(file_projection()),
        )
        .var_as(
            "version",
            g().n(NodeRef::var("canonical_version"))
                .project(file_version_projection()),
        )
        .returning(["file", "version"])
}

pub fn get_current_helix_document(
    workspace_id: String,
    file_id: String,
) -> Result<DynamicQueryRequest, String> {
    validate_workspace_file(&workspace_id, &file_id)?;
    Ok(get_current_helix_document_route(workspace_id, file_id))
}

#[register]
fn get_current_helix_document_route(workspace_id: String, file_id: String) -> ReadBatch {
    let _ = (&workspace_id, &file_id);
    read_batch()
        .var_as("canonical_file", exact_file())
        .var_as(
            "canonical_version",
            g().n(NodeRef::var("canonical_file"))
                .out(Some(CURRENT_VERSION_LABEL))
                .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                .where_(Predicate::eq_param("file_id", "file_id")),
        )
        .var_as(
            "file",
            g().n(NodeRef::var("canonical_file"))
                .project(file_projection()),
        )
        .var_as(
            "version",
            g().n(NodeRef::var("canonical_version"))
                .project(file_version_projection()),
        )
        .returning(["file", "version"])
}

pub fn get_helix_document_version(
    workspace_id: String,
    file_id: String,
    version_id: String,
) -> Result<DynamicQueryRequest, String> {
    validate_workspace_file_version(&workspace_id, &file_id, &version_id)?;
    Ok(get_helix_document_version_route(
        workspace_id,
        file_id,
        version_id,
    ))
}

#[register]
fn get_helix_document_version_route(
    workspace_id: String,
    file_id: String,
    version_id: String,
) -> ReadBatch {
    let _ = (&workspace_id, &file_id, &version_id);
    read_batch()
        .var_as("canonical_file", exact_file())
        .var_as(
            "canonical_version",
            exact_version_from_file(HAS_VERSION_LABEL),
        )
        .var_as(
            "file",
            g().n(NodeRef::var("canonical_file"))
                .project(file_projection()),
        )
        .var_as(
            "version",
            g().n(NodeRef::var("canonical_version"))
                .project(file_version_projection()),
        )
        .returning(["file", "version"])
}

pub fn get_helix_document_version_chunks(
    workspace_id: String,
    file_id: String,
    version_id: String,
) -> Result<DynamicQueryRequest, String> {
    validate_workspace_file_version(&workspace_id, &file_id, &version_id)?;
    Ok(get_helix_document_version_chunks_route(
        workspace_id,
        file_id,
        version_id,
    ))
}

#[register]
fn get_helix_document_version_chunks_route(
    workspace_id: String,
    file_id: String,
    version_id: String,
) -> ReadBatch {
    let _ = (&workspace_id, &file_id, &version_id);
    read_batch()
        .var_as("canonical_file", exact_file())
        .var_as(
            "canonical_version",
            exact_version_from_file(HAS_VERSION_LABEL),
        )
        .var_as(
            "chunks",
            g().n(NodeRef::var("canonical_version"))
                .out(Some(HAS_CHUNK_LABEL))
                .where_(Predicate::eq_param("workspace_id", "workspace_id"))
                .where_(Predicate::eq_param("file_id", "file_id"))
                .where_(Predicate::eq_param("version_id", "version_id"))
                .order_by("chunk_index", Order::Asc)
                .project(file_chunk_projection()),
        )
        .returning(["chunks"])
}

pub fn search_document_chunks_by_vector(
    search: FileChunkVectorSearch,
) -> Result<DynamicQueryRequest, String> {
    let FileChunkVectorSearch {
        workspace_id,
        query_embedding,
        limit,
    } = search;
    validate_nonempty("workspace_id", &workspace_id)?;
    if query_embedding.is_empty() {
        return Err("query embedding cannot be empty".to_string());
    }
    if query_embedding.iter().any(|value| !value.is_finite()) {
        return Err("query embedding must contain only finite values".to_string());
    }
    Ok(search_document_chunks_by_vector_route(
        workspace_id,
        query_embedding,
        search_limit_to_i64(limit)?,
    ))
}

#[register]
fn search_document_chunks_by_vector_route(
    workspace_id: String,
    query_embedding: Vec<f32>,
    limit: i64,
) -> ReadBatch {
    let _ = (&workspace_id, &query_embedding, &limit);
    read_batch()
        .var_as(
            "chunks",
            g().vector_search_nodes_with(
                FILE_CHUNK_LABEL,
                "embedding",
                PropertyInput::param("query_embedding"),
                Expr::param("limit"),
                Some(PropertyInput::param("workspace_id")),
            )
            .project(ranked_chunk_projection("$distance", "distance")),
        )
        .returning(["chunks"])
}

pub fn search_document_chunks_by_keyword(
    search: FileChunkKeywordSearch,
) -> Result<DynamicQueryRequest, String> {
    let FileChunkKeywordSearch {
        workspace_id,
        query_text,
        limit,
    } = search;
    validate_nonempty("workspace_id", &workspace_id)?;
    if query_text.trim().is_empty() {
        return Err("keyword query cannot be empty".to_string());
    }
    Ok(search_document_chunks_by_keyword_route(
        workspace_id,
        query_text,
        search_limit_to_i64(limit)?,
    ))
}

#[register]
fn search_document_chunks_by_keyword_route(
    workspace_id: String,
    query_text: String,
    limit: i64,
) -> ReadBatch {
    let _ = (&workspace_id, &query_text, &limit);
    read_batch()
        .var_as(
            "chunks",
            g().text_search_nodes_with(
                FILE_CHUNK_LABEL,
                "text",
                PropertyInput::param("query_text"),
                Expr::param("limit"),
                Some(PropertyInput::param("workspace_id")),
            )
            .project(ranked_chunk_projection("$score", "score")),
        )
        .returning(["chunks"])
}

fn exact_file() -> Traversal<OnNodes> {
    g().n_with_label(QUARRY_FILE_LABEL)
        .where_(Predicate::eq_param("workspace_id", "workspace_id"))
        .where_(Predicate::eq_param("file_id", "file_id"))
}

fn exact_version_from_file(edge_label: &str) -> Traversal<OnNodes> {
    g().n(NodeRef::var("canonical_file"))
        .out(Some(edge_label))
        .where_(Predicate::eq_param("workspace_id", "workspace_id"))
        .where_(Predicate::eq_param("file_id", "file_id"))
        .where_(Predicate::eq_param("version_id", "version_id"))
}

fn ranked_chunk_projection(
    ranking_property: &'static str,
    ranking_alias: &'static str,
) -> Vec<PropertyProjection> {
    let mut projection = file_chunk_projection();
    projection.push(PropertyProjection::renamed(ranking_property, ranking_alias));
    projection
}

fn validate_workspace_file(workspace_id: &str, file_id: &str) -> Result<(), String> {
    validate_nonempty("workspace_id", workspace_id)?;
    validate_nonempty("file_id", file_id)
}

fn validate_workspace_file_version(
    workspace_id: &str,
    file_id: &str,
    version_id: &str,
) -> Result<(), String> {
    validate_workspace_file(workspace_id, file_id)?;
    validate_nonempty("version_id", version_id)
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} cannot be empty"))
    } else {
        Ok(())
    }
}

fn search_limit_to_i64(limit: usize) -> Result<i64, String> {
    if limit == 0 {
        return Err("search limit must be greater than zero".to_string());
    }
    if limit > MAX_FILE_CHUNK_SEARCH_LIMIT {
        return Err(format!(
            "search limit must not exceed {MAX_FILE_CHUNK_SEARCH_LIMIT}"
        ));
    }
    i64::try_from(limit).map_err(|_| format!("search limit `{limit}` does not fit in i64"))
}

#[cfg(test)]
#[path = "../../../../tests/core/helix_queries/files/search_quarry_file_tests.rs"]
mod tests;
