use super::*;
use helix_db::dsl::BatchQuery;

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
fn builds_one_atomic_version_graph_write() {
    let query = insert_file_version_graph(file(), version(), vec![chunk(1), chunk(2)]).unwrap();
    let BatchQuery::Write(batch) = &query.query else {
        panic!("expected a write batch");
    };

    assert_eq!(
        batch
            .queries
            .iter()
            .filter(|entry| matches!(entry, BatchEntry::ForEach { param, .. } if param == "chunks"))
            .count(),
        1
    );
    assert_eq!(query.request_type, DynamicQueryRequestType::Write);
    assert!(query.to_json_bytes().unwrap().len() <= HELIX_MAX_QUERY_BODY_BYTES);
    assert!(matches!(
        query
            .parameters
            .as_ref()
            .and_then(|parameters| parameters.get("chunks")),
        Some(DynamicQueryValue::Array(values)) if values.len() == 2
    ));

    let json = query.to_json_string().unwrap();
    for value in [
        QUARRY_FILE_LABEL,
        FILE_VERSION_LABEL,
        FILE_CHUNK_LABEL,
        HAS_VERSION_LABEL,
        CURRENT_VERSION_LABEL,
        HAS_CHUNK_LABEL,
    ] {
        assert!(json.contains(value));
    }
    assert!(!json.contains("\"Chunk\""));
    assert!(!json.contains("document_id"));
    assert!(!json.contains("ingestion_complete"));
}

#[test]
fn cleanup_is_scoped_to_the_indexed_version() {
    let json = insert_file_version_graph(file(), version(), vec![chunk(1)])
        .unwrap()
        .to_json_string()
        .unwrap();

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
fn rejects_an_oversized_atomic_request_without_splitting() {
    let mut oversized = chunk(1);
    oversized.text = "x".repeat(HELIX_MAX_QUERY_BODY_BYTES);

    let error = insert_file_version_graph(file(), version(), vec![oversized]).unwrap_err();
    assert!(error.contains("atomic Helix file graph query"));
    assert!(error.contains(&HELIX_MAX_QUERY_BODY_BYTES.to_string()));
}

#[test]
fn indexes_versioned_file_graph_properties() {
    let json = create_document_indexes().to_json_string().unwrap();

    assert!(json.contains("file_id"));
    assert!(json.contains("version_id"));
    assert!(json.contains("chunk_id"));
    assert!(json.contains("content_sha256"));
    assert!(json.contains("embedding"));
    assert!(json.contains("workspace_id"));
    assert!(json.contains(FILE_CHUNK_LABEL));
    assert!(!json.contains("document_id"));
}

#[test]
fn uses_the_helix_v1_buffered_body_limit() {
    assert_eq!(HELIX_MAX_QUERY_BODY_BYTES, 2_097_152);
}
