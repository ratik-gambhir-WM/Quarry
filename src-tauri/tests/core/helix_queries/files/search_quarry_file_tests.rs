use super::*;

#[test]
fn vector_search_uses_runtime_embedding_limit_and_user_partition() {
    let request = search_chunks_by_vector(ChunkVectorSearch {
        user_id: "user-1".to_string(),
        query_embedding: vec![0.1, 0.2, 0.3],
        limit: 8,
    })
    .unwrap();
    let parameters = request.parameters.as_ref().unwrap();
    let json = serde_json::to_string(&request).unwrap();

    assert_eq!(
        parameters.get("user_id"),
        Some(&DynamicQueryValue::String("user-1".to_string()))
    );
    assert_eq!(parameters.get("limit"), Some(&DynamicQueryValue::I64(8)));
    assert!(parameters.contains_key("query_embedding"));
    assert!(json.contains("VectorSearchNodes"));
    assert!(json.contains("$distance"));
    assert!(json.contains("user_id"));
}

#[test]
fn keyword_search_uses_runtime_text_limit_and_user_partition() {
    let request = search_chunks_by_keyword(ChunkKeywordSearch {
        user_id: "user-1".to_string(),
        query_text: "material adverse change".to_string(),
        limit: 5,
    })
    .unwrap();
    let parameters = request.parameters.as_ref().unwrap();
    let json = serde_json::to_string(&request).unwrap();

    assert_eq!(
        parameters.get("query_text"),
        Some(&DynamicQueryValue::String(
            "material adverse change".to_string()
        ))
    );
    assert_eq!(parameters.get("limit"), Some(&DynamicQueryValue::I64(5)));
    assert!(json.contains("TextSearchNodes"));
    assert!(json.contains("$score"));
    assert!(json.contains("user_id"));
}

#[test]
fn search_inputs_are_validated() {
    assert!(search_chunks_by_vector(ChunkVectorSearch {
        user_id: "user-1".to_string(),
        query_embedding: Vec::new(),
        limit: 5,
    })
    .is_err());
    assert!(search_chunks_by_keyword(ChunkKeywordSearch {
        user_id: "user-1".to_string(),
        query_text: "   ".to_string(),
        limit: 5,
    })
    .is_err());
    assert!(search_chunks_by_keyword(ChunkKeywordSearch {
        user_id: "user-1".to_string(),
        query_text: "term".to_string(),
        limit: 0,
    })
    .is_err());
    assert_eq!(
        search_chunks_by_keyword(ChunkKeywordSearch {
            user_id: "user-1".to_string(),
            query_text: "term".to_string(),
            limit: 101,
        })
        .unwrap_err(),
        "search limit cannot exceed 100"
    );
}

#[test]
fn document_lookup_is_user_scoped_and_requires_completed_ingestion() {
    let request =
        find_quarry_file_by_content_hash("user-1".to_string(), "content-hash".to_string()).unwrap();
    let json = serde_json::to_string(&request).unwrap();

    assert!(json.contains("user_id"));
    assert!(json.contains("content_hash"));
    assert!(json.contains("ingestion_complete"));
}

#[test]
fn registered_bundle_contains_chunk_search_routes() {
    let bundle = helix_db::query_generator::build_query_bundle().unwrap();

    for route in [
        "search_chunks_by_vector_route",
        "search_chunks_by_keyword_route",
    ] {
        assert!(bundle.read_routes.contains_key(route));
    }
}
