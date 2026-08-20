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
