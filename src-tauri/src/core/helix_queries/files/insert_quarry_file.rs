use helix_db::dsl::prelude::*;

use crate::core::nodes::document_node::{ChunkNode, DocumentNode};

pub const QUARRY_FILE_LABEL: &str = "QuarryFile";
pub const CHUNK_LABEL: &str = "Chunk";
pub const QUARRY_FILE_HAS_CHUNK_LABEL: &str = "HAS_CHUNK";
pub const INGESTION_COMPLETE_PROPERTY: &str = "ingestion_complete";

/// Creates the QuarryFile node once before its chunks are processed.
pub fn insert_quarry_file(document: DocumentNode) -> Result<DynamicQueryRequest, String> {
    let DocumentNode {
        document_id,
        user_id,
        file_name,
        source_type,
        local_path,
        file_size_bytes,
        token_count,
        content_hash,
        rendered_pdf_path,
    } = document;

    Ok(insert_quarry_file_route(
        document_id,
        user_id,
        file_name,
        source_type,
        local_path.map_or(PropertyValue::Null, PropertyValue::String),
        u64_to_i64(file_size_bytes, "file_size_bytes")?,
        u64_to_i64(token_count, "token_count")?,
        content_hash,
        rendered_pdf_path.unwrap_or_default(),
    ))
}

#[allow(clippy::too_many_arguments)]
#[register]
fn insert_quarry_file_route(
    document_id: String,
    user_id: String,
    file_name: String,
    source_type: String,
    local_path: PropertyValue,
    file_size_bytes: i64,
    token_count: i64,
    content_hash: String,
    rendered_pdf_path: String,
) -> WriteBatch {
    let _ = (
        &document_id,
        &user_id,
        &file_name,
        &source_type,
        &local_path,
        &file_size_bytes,
        &token_count,
        &content_hash,
        &rendered_pdf_path,
    );

    write_batch()
        .var_as(
            "stale_chunks",
            g().n_with_label(CHUNK_LABEL)
                .where_(Predicate::eq_param("document_id", "document_id"))
                .where_(Predicate::eq_param("user_id", "user_id"))
                .drop(),
        )
        .var_as(
            "stale_quarry_file",
            g().n_with_label(QUARRY_FILE_LABEL)
                .where_(Predicate::eq_param("document_id", "document_id"))
                .where_(Predicate::eq_param("user_id", "user_id"))
                .drop(),
        )
        .var_as(
            "quarry_file",
            g().add_n(
                QUARRY_FILE_LABEL,
                vec![
                    ("document_id", PropertyInput::param("document_id")),
                    ("user_id", PropertyInput::param("user_id")),
                    ("file_name", PropertyInput::param("file_name")),
                    ("source_type", PropertyInput::param("source_type")),
                    ("local_path", PropertyInput::param("local_path")),
                    ("file_size_bytes", PropertyInput::param("file_size_bytes")),
                    ("token_count", PropertyInput::param("token_count")),
                    ("content_hash", PropertyInput::param("content_hash")),
                    (
                        "rendered_pdf_path",
                        PropertyInput::param("rendered_pdf_path"),
                    ),
                    (INGESTION_COMPLETE_PROPERTY, PropertyInput::from(false)),
                ],
            )
            .project(quarry_file_projection()),
        )
        .returning(["quarry_file"])
}

pub fn mark_quarry_file_ingestion_complete(
    document_id: String,
    user_id: String,
) -> Result<DynamicQueryRequest, String> {
    if document_id.trim().is_empty() {
        return Err("document_id cannot be empty".to_string());
    }
    if user_id.trim().is_empty() {
        return Err("user_id cannot be empty".to_string());
    }

    Ok(mark_quarry_file_ingestion_complete_route(
        document_id,
        user_id,
    ))
}

#[register]
fn mark_quarry_file_ingestion_complete_route(document_id: String, user_id: String) -> WriteBatch {
    let _ = (&document_id, &user_id);
    write_batch()
        .var_as(
            "quarry_file",
            g().n_with_label(QUARRY_FILE_LABEL)
                .where_(Predicate::eq_param("document_id", "document_id"))
                .where_(Predicate::eq_param("user_id", "user_id"))
                .set_property(INGESTION_COMPLETE_PROPERTY, true)
                .project(quarry_file_projection()),
        )
        .returning(["quarry_file"])
}

/// Finds an existing QuarryFile, creates a chunk, and connects them.
///
/// The conditional entries prevent creation of an orphan chunk when the
/// referenced QuarryFile does not exist for the supplied document and user.
pub fn insert_chunk_for_document(chunk: ChunkNode) -> Result<DynamicQueryRequest, String> {
    let ChunkNode {
        chunk_id,
        document_id,
        user_id,
        text,
        embedding,
        sequence_number,
        page_numbers,
        start_offset,
        end_offset,
        token_count,
        content_hash,
        section_title,
    } = chunk;
    let embedding =
        embedding.ok_or_else(|| format!("chunk `{chunk_id}` does not contain an embedding"))?;

    Ok(insert_chunk_for_document_route(
        document_id,
        user_id,
        chunk_id,
        text,
        embedding,
        i64::from(sequence_number),
        page_numbers.map_or(PropertyValue::Null, |page_numbers| {
            PropertyValue::I64Array(page_numbers.into_iter().map(i64::from).collect())
        }),
        usize_to_i64(start_offset, "start_offset")?,
        usize_to_i64(end_offset, "end_offset")?,
        i64::from(token_count),
        content_hash,
        section_title.unwrap_or_default(),
    ))
}

#[allow(clippy::too_many_arguments)]
#[register]
fn insert_chunk_for_document_route(
    document_id: String,
    user_id: String,
    chunk_id: String,
    text: String,
    embedding: Vec<f32>,
    sequence_number: i64,
    page_numbers: PropertyValue,
    start_offset: i64,
    end_offset: i64,
    token_count: i64,
    content_hash: String,
    section_title: String,
) -> WriteBatch {
    let _ = (
        &document_id,
        &user_id,
        &chunk_id,
        &text,
        &embedding,
        &sequence_number,
        &page_numbers,
        &start_offset,
        &end_offset,
        &token_count,
        &content_hash,
        &section_title,
    );

    write_batch()
        .var_as(
            "quarry_file",
            g().n_with_label(QUARRY_FILE_LABEL)
                .where_(Predicate::eq_param("document_id", "document_id"))
                .where_(Predicate::eq_param("user_id", "user_id")),
        )
        .var_as_if(
            "chunk",
            BatchCondition::VarNotEmpty("quarry_file".to_string()),
            g().add_n(
                CHUNK_LABEL,
                vec![
                    ("chunk_id", PropertyInput::param("chunk_id")),
                    ("document_id", PropertyInput::param("document_id")),
                    ("user_id", PropertyInput::param("user_id")),
                    ("text", PropertyInput::param("text")),
                    ("embedding", PropertyInput::param("embedding")),
                    ("sequence_number", PropertyInput::param("sequence_number")),
                    ("page_numbers", PropertyInput::param("page_numbers")),
                    ("start_offset", PropertyInput::param("start_offset")),
                    ("end_offset", PropertyInput::param("end_offset")),
                    ("token_count", PropertyInput::param("token_count")),
                    ("content_hash", PropertyInput::param("content_hash")),
                    ("section_title", PropertyInput::param("section_title")),
                ],
            ),
        )
        .var_as_if(
            "quarry_file_has_chunk",
            BatchCondition::VarNotEmpty("chunk".to_string()),
            g().n(NodeRef::var("quarry_file")).add_e(
                QUARRY_FILE_HAS_CHUNK_LABEL,
                NodeRef::var("chunk"),
                vec![("user_id", PropertyInput::param("user_id"))],
            ),
        )
        .returning(["quarry_file", "chunk", "quarry_file_has_chunk"])
}

/// Creates all indexes needed for QuarryFile lookup and chunk search.
#[register]
pub fn create_document_indexes() -> WriteBatch {
    write_batch()
        .var_as(
            "document_id_unique",
            g().create_index_if_not_exists(IndexSpec::node_unique_equality(
                QUARRY_FILE_LABEL,
                "document_id",
            )),
        )
        .var_as(
            "document_user_id",
            g().create_index_if_not_exists(IndexSpec::node_equality(QUARRY_FILE_LABEL, "user_id")),
        )
        .var_as(
            "document_source_type",
            g().create_index_if_not_exists(IndexSpec::node_equality(
                QUARRY_FILE_LABEL,
                "source_type",
            )),
        )
        .var_as(
            "document_local_path",
            g().create_index_if_not_exists(IndexSpec::node_equality(
                QUARRY_FILE_LABEL,
                "local_path",
            )),
        )
        .var_as(
            "document_content_hash",
            g().create_index_if_not_exists(IndexSpec::node_equality(
                QUARRY_FILE_LABEL,
                "content_hash",
            )),
        )
        .var_as(
            "document_file_name",
            g().create_index_if_not_exists(IndexSpec::node_text(
                QUARRY_FILE_LABEL,
                "file_name",
                None::<&str>,
            )),
        )
        .var_as(
            "chunk_id_unique",
            g().create_index_if_not_exists(IndexSpec::node_unique_equality(
                CHUNK_LABEL,
                "chunk_id",
            )),
        )
        .var_as(
            "chunk_document_id",
            g().create_index_if_not_exists(IndexSpec::node_equality(CHUNK_LABEL, "document_id")),
        )
        .var_as(
            "chunk_user_id",
            g().create_index_if_not_exists(IndexSpec::node_equality(CHUNK_LABEL, "user_id")),
        )
        .var_as(
            "chunk_embedding",
            g().create_index_if_not_exists(IndexSpec::node_vector(
                CHUNK_LABEL,
                "embedding",
                Some("user_id"),
            )),
        )
        .var_as(
            "chunk_text",
            g().create_index_if_not_exists(IndexSpec::node_text(
                CHUNK_LABEL,
                "text",
                Some("user_id"),
            )),
        )
}

fn quarry_file_projection() -> Vec<PropertyProjection> {
    vec![
        PropertyProjection::renamed("$id", "id"),
        PropertyProjection::new("document_id"),
        PropertyProjection::new("user_id"),
        PropertyProjection::new("file_name"),
        PropertyProjection::new("source_type"),
        PropertyProjection::new("local_path"),
        PropertyProjection::new("file_size_bytes"),
        PropertyProjection::new("token_count"),
        PropertyProjection::new("content_hash"),
        PropertyProjection::new("rendered_pdf_path"),
    ]
}

fn usize_to_i64(value: usize, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} value `{value}` does not fit in i64"))
}

fn u64_to_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} value `{value}` does not fit in i64"))
}

#[cfg(test)]
#[path = "../../../../tests/core/helix_queries/files/insert_quarry_file_tests.rs"]
mod tests;
