use super::*;

#[test]
fn vector_search_requires_an_embedding() {
    let error = search_chunks_by_vector(ChunkVectorSearch {
        user_id: "user-1".to_string(),
        query_embedding: Vec::new(),
        limit: 10,
    })
    .unwrap_err();

    assert_eq!(error, "query embedding cannot be empty");
}

#[test]
fn keyword_search_requires_a_positive_limit() {
    let error = search_chunks_by_keyword(ChunkKeywordSearch {
        user_id: "user-1".to_string(),
        query_text: "revenue growth".to_string(),
        limit: 0,
    })
    .unwrap_err();

    assert_eq!(error, "search limit must be greater than zero");
}

#[test]
fn document_lookup_requires_user_and_content_hash() {
    let query =
        find_quarry_file_by_content_hash("user-1".to_string(), "content-hash".to_string()).unwrap();
    let json = query.to_json_string().unwrap();
    assert!(json.contains(INGESTION_COMPLETE_PROPERTY));
    assert!(json.contains("true"));
    assert_eq!(
        find_quarry_file_by_content_hash("user-1".to_string(), " ".to_string()).unwrap_err(),
        "content_hash cannot be empty"
    );
}
