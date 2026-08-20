use helix_db::dsl::prelude::*;

use crate::core::nodes::document_node::{ChunkNode, DocumentNode};

pub const QUARRY_FILE_LABEL: &str = "QuarryFile";
pub const CHUNK_LABEL: &str = "Chunk";
pub const QUARRY_FILE_HAS_CHUNK_LABEL: &str = "HAS_CHUNK";
pub const INGESTION_COMPLETE_PROPERTY: &str = "ingestion_complete";

// Helix's /v1/query route uses Axum's default buffered-body limit of 2 MiB.
pub const HELIX_MAX_QUERY_BODY_BYTES: usize = 2_097_152;

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

pub fn insert_chunk_batches(chunks: &[ChunkNode]) -> Result<Vec<DynamicQueryRequest>, String> {
    insert_chunk_batches_with_limit(chunks, HELIX_MAX_QUERY_BODY_BYTES)
}

#[register]
fn insert_chunks_for_document_route(chunks: Vec<ParamObject>) -> WriteBatch {
    let _ = &chunks;
    write_batch()
        .for_each_param(
            "chunks",
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
                ),
        )
        .returning(["quarry_file", "chunk", "quarry_file_has_chunk"])
}

fn insert_chunk_batches_with_limit(
    chunks: &[ChunkNode],
    max_payload_bytes: usize,
) -> Result<Vec<DynamicQueryRequest>, String> {
    let mut batches = Vec::new();
    let mut remaining = chunks;

    while !remaining.is_empty() {
        let (chunk_count, query) = largest_chunk_batch_that_fits(remaining, max_payload_bytes)?;
        batches.push(query);
        remaining = &remaining[chunk_count..];
    }

    Ok(batches)
}

fn largest_chunk_batch_that_fits(
    chunks: &[ChunkNode],
    max_payload_bytes: usize,
) -> Result<(usize, DynamicQueryRequest), String> {
    let mut low = 1usize;
    let mut high = chunks.len();
    let mut best = None;
    let mut smallest_payload_bytes = None;

    while low <= high {
        let chunk_count = low + (high - low) / 2;
        let query = build_chunk_batch(&chunks[..chunk_count])?;
        let payload_bytes = query
            .to_json_bytes()
            .map_err(|error| format!("failed to serialize Helix chunk batch: {error}"))?
            .len();
        if chunk_count == 1 {
            smallest_payload_bytes = Some(payload_bytes);
        }

        if payload_bytes <= max_payload_bytes {
            best = Some((chunk_count, query));
            low = chunk_count + 1;
        } else {
            high = chunk_count - 1;
        }
    }

    best.ok_or_else(|| {
        let payload_bytes = smallest_payload_bytes.unwrap_or_else(|| {
            build_chunk_batch(&chunks[..1])
                .and_then(|query| {
                    query
                        .to_json_bytes()
                        .map_err(|error| error.to_string())
                })
                .map(|payload| payload.len())
                .unwrap_or_default()
        });
        format!(
            "chunk `{}` requires a {payload_bytes}-byte Helix query payload, exceeding the {max_payload_bytes}-byte limit",
            chunks[0].chunk_id
        )
    })
}

fn build_chunk_batch(chunks: &[ChunkNode]) -> Result<DynamicQueryRequest, String> {
    let params = chunks
        .iter()
        .map(chunk_params)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(insert_chunks_for_document_route(params))
}

fn chunk_params(chunk: &ChunkNode) -> Result<ParamObject, String> {
    let embedding = chunk
        .embedding
        .clone()
        .ok_or_else(|| format!("chunk `{}` does not contain an embedding", chunk.chunk_id))?;

    Ok(ParamObject::from([
        (
            "chunk_id".to_string(),
            PropertyValue::String(chunk.chunk_id.clone()),
        ),
        (
            "document_id".to_string(),
            PropertyValue::String(chunk.document_id.clone()),
        ),
        (
            "user_id".to_string(),
            PropertyValue::String(chunk.user_id.clone()),
        ),
        (
            "text".to_string(),
            PropertyValue::String(chunk.text.clone()),
        ),
        ("embedding".to_string(), PropertyValue::F32Array(embedding)),
        (
            "sequence_number".to_string(),
            PropertyValue::I64(i64::from(chunk.sequence_number)),
        ),
        (
            "page_numbers".to_string(),
            chunk
                .page_numbers
                .as_ref()
                .map_or(PropertyValue::Null, |values| {
                    PropertyValue::I64Array(values.iter().copied().map(i64::from).collect())
                }),
        ),
        (
            "start_offset".to_string(),
            PropertyValue::I64(usize_to_i64(chunk.start_offset, "start_offset")?),
        ),
        (
            "end_offset".to_string(),
            PropertyValue::I64(usize_to_i64(chunk.end_offset, "end_offset")?),
        ),
        (
            "token_count".to_string(),
            PropertyValue::I64(i64::from(chunk.token_count)),
        ),
        (
            "content_hash".to_string(),
            PropertyValue::String(chunk.content_hash.clone()),
        ),
        (
            "section_title".to_string(),
            PropertyValue::String(chunk.section_title.clone().unwrap_or_default()),
        ),
    ]))
}

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
        PropertyProjection::new(INGESTION_COMPLETE_PROPERTY),
    ]
}

fn usize_to_i64(value: usize, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} value `{value}` does not fit in i64"))
}

fn u64_to_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} value `{value}` does not fit in i64"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use helix_db::dsl::BatchQuery;

    fn document() -> DocumentNode {
        DocumentNode {
            document_id: "doc-1".to_string(),
            user_id: "user-1".to_string(),
            file_name: "test.pdf".to_string(),
            source_type: "pdf".to_string(),
            local_path: None,
            file_size_bytes: 4,
            token_count: 2,
            content_hash: "document-hash".to_string(),
            rendered_pdf_path: None,
        }
    }

    fn chunk(index: u32) -> ChunkNode {
        ChunkNode {
            chunk_id: format!("chunk-{index}"),
            document_id: "doc-1".to_string(),
            user_id: "user-1".to_string(),
            text: format!("text-{index}"),
            embedding: Some(vec![index as f32, 1.0]),
            sequence_number: index,
            page_numbers: Some(vec![index]),
            start_offset: 0,
            end_offset: 6,
            token_count: 1,
            content_hash: format!("hash-{index}"),
            section_title: None,
        }
    }

    #[test]
    fn replaces_an_incomplete_document_before_inserting() {
        let query = insert_quarry_file(document()).unwrap();
        let BatchQuery::Write(batch) = &query.query else {
            panic!("expected a write batch");
        };

        assert_eq!(batch.queries.len(), 3);
        assert_eq!(batch.returns, vec!["quarry_file"]);
        let json = query.to_json_string().unwrap();
        assert!(json.contains(QUARRY_FILE_LABEL));
        assert!(json.contains(INGESTION_COMPLETE_PROPERTY));
    }

    #[test]
    fn uses_the_helix_v1_buffered_body_limit() {
        assert_eq!(HELIX_MAX_QUERY_BODY_BYTES, 2_097_152);
    }

    #[test]
    fn marks_document_ingestion_complete() {
        let query =
            mark_quarry_file_ingestion_complete("doc-1".to_string(), "user-1".to_string()).unwrap();
        let json = query.to_json_string().unwrap();

        assert!(json.contains(INGESTION_COMPLETE_PROPERTY));
        assert!(json.contains("true"));
    }

    #[test]
    fn builds_one_chunk_and_edge_write_for_each_batched_chunk() {
        let mut chunks = vec![chunk(1), chunk(2)];
        for chunk in &mut chunks {
            chunk.embedding = Some(vec![0.25; 1_536]);
        }
        let queries = insert_chunk_batches(&chunks).unwrap();
        assert_eq!(queries.len(), 1);
        let query = &queries[0];
        let BatchQuery::Write(batch) = &query.query else {
            panic!("expected a write batch");
        };

        assert_eq!(batch.queries.len(), 1);
        assert!(matches!(
            &batch.queries[0],
            BatchEntry::ForEach { param, body } if param == "chunks" && body.len() == 3
        ));
        assert_eq!(
            batch.returns,
            vec!["quarry_file", "chunk", "quarry_file_has_chunk"]
        );
        let json = query.to_json_string().unwrap();
        assert!(json.contains(CHUNK_LABEL));
        assert!(json.contains(QUARRY_FILE_HAS_CHUNK_LABEL));
        assert!(json.len() <= HELIX_MAX_QUERY_BODY_BYTES);
        assert!(matches!(
            query
                .parameters
                .as_ref()
                .and_then(|parameters| parameters.get("chunks")),
            Some(DynamicQueryValue::Array(values)) if values.len() == 2
        ));
    }

    #[test]
    fn splits_chunk_batches_at_the_serialized_payload_limit() {
        let chunks = vec![chunk(1), chunk(2), chunk(3)];
        let two_chunk_payload_bytes = build_chunk_batch(&chunks[..2])
            .unwrap()
            .to_json_bytes()
            .unwrap()
            .len();

        let queries = insert_chunk_batches_with_limit(&chunks, two_chunk_payload_bytes).unwrap();

        assert_eq!(queries.len(), 2);
        assert!(queries
            .iter()
            .all(|query| { query.to_json_bytes().unwrap().len() <= two_chunk_payload_bytes }));
    }

    #[test]
    fn rejects_a_chunk_without_an_embedding() {
        let mut invalid_chunk = chunk(1);
        invalid_chunk.embedding = None;

        let error = insert_chunk_batches(&[invalid_chunk]).unwrap_err();
        assert!(error.contains("does not contain an embedding"));
    }

    #[test]
    fn rejects_a_single_chunk_larger_than_the_payload_limit() {
        let chunk = chunk(1);
        let payload_bytes = build_chunk_batch(std::slice::from_ref(&chunk))
            .unwrap()
            .to_json_bytes()
            .unwrap()
            .len();

        let error = insert_chunk_batches_with_limit(&[chunk], payload_bytes - 1).unwrap_err();

        assert!(error.contains("exceeding"));
        assert!(error.contains(&(payload_bytes - 1).to_string()));
    }
}
