use super::*;

#[test]
fn vector_search_is_workspace_partitioned_and_validated() {
    let query = search_document_chunks_by_vector(FileChunkVectorSearch {
        workspace_id: "workspace-1".to_string(),
        query_embedding: vec![0.25, 0.5],
        limit: 10,
    })
    .unwrap();
    let json = query.to_json_string().unwrap();
    assert!(json.contains(FILE_CHUNK_LABEL));
    assert!(json.contains("workspace_id"));
    assert!(json.contains("$distance"));
    assert!(!json.contains("user_id"));
    assert!(!json.contains("\"embedding\",\"alias\":null"));

    assert_eq!(
        search_document_chunks_by_vector(FileChunkVectorSearch {
            workspace_id: "workspace-1".to_string(),
            query_embedding: Vec::new(),
            limit: 10,
        })
        .unwrap_err(),
        "query embedding cannot be empty"
    );
    assert!(search_document_chunks_by_vector(FileChunkVectorSearch {
        workspace_id: "workspace-1".to_string(),
        query_embedding: vec![f32::NAN],
        limit: 10,
    })
    .unwrap_err()
    .contains("finite"));
}

#[test]
fn keyword_search_requires_a_bounded_positive_limit() {
    assert_eq!(
        search_document_chunks_by_keyword(FileChunkKeywordSearch {
            workspace_id: "workspace-1".to_string(),
            query_text: "revenue growth".to_string(),
            limit: 0,
        })
        .unwrap_err(),
        "search limit must be greater than zero"
    );
    assert!(search_document_chunks_by_keyword(FileChunkKeywordSearch {
        workspace_id: "workspace-1".to_string(),
        query_text: "revenue growth".to_string(),
        limit: MAX_FILE_CHUNK_SEARCH_LIMIT + 1,
    })
    .unwrap_err()
    .contains("must not exceed"));
}

#[test]
fn current_and_historical_queries_use_the_correct_edges() {
    let current = get_current_helix_document("workspace-1".to_string(), "file-1".to_string())
        .unwrap()
        .to_json_string()
        .unwrap();
    assert!(current.contains(CURRENT_VERSION_LABEL));
    assert!(!current.contains(HAS_VERSION_LABEL));

    let historical = get_helix_document_version(
        "workspace-1".to_string(),
        "file-1".to_string(),
        "version-1".to_string(),
    )
    .unwrap()
    .to_json_string()
    .unwrap();
    assert!(historical.contains(HAS_VERSION_LABEL));
    assert!(!historical.contains(CURRENT_VERSION_LABEL));
}

#[test]
fn content_lookup_uses_current_version_and_content_hash() {
    let query = find_current_helix_document_by_content_hash(
        "workspace-1".to_string(),
        "content-hash".to_string(),
    )
    .unwrap();
    let json = query.to_json_string().unwrap();
    assert!(json.contains(CURRENT_VERSION_LABEL));
    assert!(json.contains("content_sha256"));
    assert!(!json.contains("document_id"));
}

#[test]
fn version_chunks_are_scoped_and_ordered() {
    let query = get_helix_document_version_chunks(
        "workspace-1".to_string(),
        "file-1".to_string(),
        "version-1".to_string(),
    )
    .unwrap();
    let json = query.to_json_string().unwrap();
    assert!(json.contains(HAS_VERSION_LABEL));
    assert!(json.contains(HAS_CHUNK_LABEL));
    assert!(json.contains("chunk_index"));
    assert!(!json.contains("embedding"));
}

#[test]
fn legacy_user_id_request_field_is_rejected() {
    let error = serde_json::from_value::<FileChunkKeywordSearch>(serde_json::json!({
        "userId": "legacy",
        "queryText": "query",
        "limit": 10
    }))
    .unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}
