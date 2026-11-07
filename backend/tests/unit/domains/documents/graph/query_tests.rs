use super::*;

#[test]
fn vector_search_is_deal_id_partitioned_and_validated() {
    let query = search_document_chunks_by_vector(FileChunkVectorSearch {
        deal_id: "deal_id-1".to_string(),
        query_embedding: vec![0.25, 0.5],
        limit: 10,
    })
    .unwrap();
    let json = query.to_json_string().unwrap();
    assert!(json.contains(FILE_CHUNK_LABEL));
    assert!(json.contains("deal_id"));
    assert!(json.contains("$distance"));
    assert!(!json.contains("user_id"));
    assert!(!json.contains("\"embedding\",\"alias\":null"));

    assert_eq!(
        search_document_chunks_by_vector(FileChunkVectorSearch {
            deal_id: "deal_id-1".to_string(),
            query_embedding: Vec::new(),
            limit: 10,
        })
        .unwrap_err(),
        "query embedding cannot be empty"
    );
    assert!(search_document_chunks_by_vector(FileChunkVectorSearch {
        deal_id: "deal_id-1".to_string(),
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
            deal_id: "deal_id-1".to_string(),
            query_text: "revenue growth".to_string(),
            limit: 0,
        })
        .unwrap_err(),
        "search limit must be greater than zero"
    );
    assert!(search_document_chunks_by_keyword(FileChunkKeywordSearch {
        deal_id: "deal_id-1".to_string(),
        query_text: "revenue growth".to_string(),
        limit: MAX_FILE_CHUNK_SEARCH_LIMIT + 1,
    })
    .unwrap_err()
    .contains("must not exceed"));
}

#[test]
fn current_and_historical_queries_use_the_correct_edges() {
    let current = get_current_helix_document("deal_id-1".to_string(), "file-1".to_string())
        .unwrap()
        .to_json_string()
        .unwrap();
    assert!(current.contains(CURRENT_VERSION_LABEL));
    assert!(!current.contains(HAS_VERSION_LABEL));

    let historical = get_helix_document_version(
        "deal_id-1".to_string(),
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
        "deal_id-1".to_string(),
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
        "deal_id-1".to_string(),
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

#[test]
fn current_vector_query_binds_candidates_before_rank_and_projects_distance() {
    let query = current_vector_candidates(
        "DEAL-SYNTHETIC".into(),
        vec!["synthetic-a".into(), "synthetic-b".into()],
        vec![1.0, 0.0],
        5,
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&query.to_json_string().unwrap()).unwrap();
    let project = &json["query"]["read"]["entries"][0]["query"]["root"]["project"];
    let within = &project["input"]["vector_search_nodes_within"];
    assert_eq!(within["label"], "FileChunk");
    assert_eq!(within["property"], "embedding");
    assert_eq!(within["query_vector"]["expr"]["param"], "query_embedding");
    assert_eq!(within["tenant_value"]["expr"]["param"], "deal_id");
    assert_eq!(
        within["input"]["where"]["predicate"]["is_in"]["value"]["property"],
        "chunk_id"
    );
    assert_eq!(
        within["input"]["where"]["predicate"]["is_in"]["values"]["param"],
        "chunk_ids"
    );
    assert_eq!(
        within["input"]["where"]["input"]["where"]["predicate"]["eq"]["right"]["param"],
        "deal_id"
    );
    assert_eq!(
        json["parameters"]["query_embedding"],
        serde_json::json!([1.0, 0.0])
    );
    assert_eq!(
        json["parameters"]["chunk_ids"],
        serde_json::json!(["synthetic-a", "synthetic-b"])
    );
    assert_eq!(within["k"]["expr"]["param"], "limit");
    assert!(project["projections"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!({"property":{"alias":"distance","source":"$distance"}})));
}

#[test]
fn helix_normalized_cosine_distance_converts_to_actual_similarity() {
    for (distance, similarity) in [(0.0, 1.0), (0.2, 0.6), (0.5, 0.0), (1.0, -1.0)] {
        assert!((helix_cosine_similarity(distance).unwrap() - similarity).abs() < 1e-12);
    }
    for distance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.01] {
        assert!(helix_cosine_similarity(distance).is_none());
    }
}
