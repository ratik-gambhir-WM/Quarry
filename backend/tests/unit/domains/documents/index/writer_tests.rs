use super::*;
use std::num::NonZeroUsize;

use helix_db::dsl::{BatchQuery, QueryRequestType, QueryValue};

fn file() -> FileNode {
    FileNode {
        workspace_id: "workspace-1".to_string(),
        file_id: "file-1".to_string(),
        display_name: "report.pdf".to_string(),
    }
}

fn version() -> FileVersionNode {
    FileVersionNode {
        workspace_id: "workspace-1".to_string(),
        file_id: "file-1".to_string(),
        version_id: "version-1".to_string(),
        mime_type: "application/pdf".to_string(),
        content_sha256: "document-hash".to_string(),
        byte_size: 4,
        index_generation: "version-1".to_string(),
        indexed_at: "2026-08-26T00:00:00.000Z".to_string(),
    }
}

fn chunk(index: i64) -> FileChunkNode {
    FileChunkNode {
        chunk_id: format!("chunk-{index}"),
        workspace_id: "workspace-1".to_string(),
        file_id: "file-1".to_string(),
        version_id: "version-1".to_string(),
        index_generation: "version-1".to_string(),
        chunk_index: index,
        text: format!("text-{index}"),
        embedding: vec![index as f32, 1.0],
        chunk_sha256: format!("hash-{index}"),
        token_count: 1,
        page_start: Some(index),
        page_end: Some(index),
        char_start: 0,
        char_end: 6,
        section_path: String::new(),
        created_at: "2026-08-26T00:00:00.000Z".to_string(),
    }
}

#[test]
fn builds_bounded_version_graph_writes() {
    let graph = insert_file_version_graph(file(), version(), vec![chunk(1), chunk(2)]).unwrap();
    let BatchQuery::Write(batch) = graph.setup_query.query() else {
        panic!("expected a write batch");
    };

    assert_eq!(
        batch
            .entries
            .iter()
            .filter(|entry| matches!(entry, BatchEntry::ForEach { param, .. } if param == "chunks"))
            .count(),
        0
    );
    assert_eq!(graph.setup_query.request_type(), QueryRequestType::Write);
    assert!(graph.setup_query.to_json_bytes().unwrap().len() <= HELIX_MAX_QUERY_BODY_BYTES);
    assert_eq!(graph.chunk_queries.len(), 1);
    let chunk_query = &graph.chunk_queries[0];
    assert_eq!(chunk_query.request_type(), QueryRequestType::Write);
    assert!(chunk_query.to_json_bytes().unwrap().len() <= HELIX_MAX_QUERY_BODY_BYTES);
    assert!(matches!(
        chunk_query
            .parameters()
            .and_then(|parameters| parameters.get("chunks")),
        Some(QueryValue::Array(values)) if values.len() == 2
    ));

    let setup_json = graph.setup_query.to_json_string().unwrap();
    let chunk_json = chunk_query.to_json_string().unwrap();
    for value in [
        QUARRY_FILE_LABEL,
        FILE_VERSION_LABEL,
        HAS_VERSION_LABEL,
        CURRENT_VERSION_LABEL,
    ] {
        assert!(setup_json.contains(value));
    }
    for value in [FILE_CHUNK_LABEL, HAS_CHUNK_LABEL] {
        assert!(chunk_json.contains(value));
    }
    assert!(!setup_json.contains("\"Chunk\""));
    assert!(!chunk_json.contains("document_id"));
    assert!(!chunk_json.contains("ingestion_complete"));
}

#[test]
fn cleanup_is_scoped_to_the_indexed_version() {
    let graph = insert_file_version_graph(file(), version(), vec![chunk(1)]).unwrap();
    let json = graph.setup_query.to_json_string().unwrap();

    assert!(json.contains("removed_version_chunks"));
    assert!(json.contains("version_id"));
    assert!(!json.contains("stale_quarry_file"));
    assert!(!json.contains("drop\":{\"input\":{\"n\":{\"ids\":{\"var\":\"canonical_version"));
}

#[test]
fn rejects_mismatched_graph_identity() {
    let mut invalid = chunk(1);
    invalid.version_id = "another-version".to_string();

    let error = insert_file_version_graph(file(), version(), vec![invalid]).unwrap_err();
    assert!(error.contains("does not match"));
}

#[test]
fn rejects_a_chunk_that_cannot_fit_in_its_own_request() {
    let mut oversized = chunk(1);
    oversized.text = "x".repeat(HELIX_MAX_QUERY_BODY_BYTES);

    let error = insert_file_version_graph(file(), version(), vec![oversized]).unwrap_err();
    assert!(error.contains("Helix file chunk batch query"));
    assert!(error.contains(&HELIX_MAX_QUERY_BODY_BYTES.to_string()));
}

#[test]
fn splits_large_graph_chunks_into_individually_safe_requests() {
    let chunks = (1..=3)
        .map(|index| {
            let mut chunk = chunk(index);
            chunk.text = "x".repeat(900_000);
            chunk
        })
        .collect();

    let graph = insert_file_version_graph(file(), version(), chunks).unwrap();

    assert!(graph.chunk_queries.len() > 1);
    assert!(graph
        .chunk_queries
        .iter()
        .all(|query| query.to_json_bytes().unwrap().len() <= HELIX_MAX_QUERY_BODY_BYTES));
}

#[test]
fn indexes_versioned_file_graph_properties() {
    let query = create_document_indexes(NonZeroUsize::new(1536).unwrap());
    let json = query.to_json_string().unwrap();

    assert!(json.contains("file_id"));
    assert!(json.contains("version_id"));
    assert!(json.contains("chunk_id"));
    assert!(json.contains("content_sha256"));
    assert!(json.contains("embedding"));
    assert!(json.contains("workspace_id"));
    assert!(json.contains(FILE_CHUNK_LABEL));
    assert!(json.contains("cosine"));
    assert!(!json.contains("document_id"));
    assert_eq!(DOCUMENT_INDEX_NAMES.len(), 13);
    assert_eq!(query.request_type(), QueryRequestType::Write);
}

#[test]
fn reads_index_operation_status_with_a_read_query() {
    let query = get_document_index_operation("operation-1".to_string());

    assert_eq!(query.request_type(), QueryRequestType::Read);
    assert!(query
        .to_json_string()
        .unwrap()
        .contains("get_index_operation"));
}

#[test]
fn uses_the_conservative_helix_v3_preflight_body_limit() {
    assert_eq!(HELIX_MAX_QUERY_BODY_BYTES, 2_097_152);
}
