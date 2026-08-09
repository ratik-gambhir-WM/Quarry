use std::fs;

use super::*;

#[test]
fn document_job_request_rejects_blank_users_and_ungranted_paths() {
    let state = AppState::new_for_test().unwrap();
    let error = validate_start_request(
        &state,
        StartDocumentJobsInput {
            paths: vec!["/tmp/report.pdf".into()],
            user_id: "  ".into(),
        },
    )
    .unwrap_err();
    assert_eq!(error, "userId is required");
}

#[test]
fn native_selection_grant_is_required_and_supported_files_validate() {
    let state = AppState::new_for_test().unwrap();
    let test_dir = std::env::temp_dir().join(format!(
        "quarry-document-command-test-{}",
        std::process::id()
    ));
    fs::create_dir_all(&test_dir).unwrap();
    let path = test_dir.join("memo.pdf");
    fs::write(&path, b"fixture").unwrap();

    let ungranted = validate_start_request(
        &state,
        StartDocumentJobsInput {
            paths: vec![path.display().to_string()],
            user_id: "user-1".into(),
        },
    )
    .unwrap_err();
    assert_eq!(
        ungranted,
        "memo.pdf is not authorized by the native file picker"
    );
    assert_eq!(
        describe_granted_document_files(&state, vec![path.display().to_string()]).unwrap_err(),
        "memo.pdf was not received from a native file drop"
    );

    let canonical = path.canonicalize().unwrap();
    state.grant_paths([canonical.clone()]).unwrap();
    let files = validate_start_request(
        &state,
        StartDocumentJobsInput {
            paths: vec![path.display().to_string()],
            user_id: "user-1".into(),
        },
    )
    .unwrap();
    assert_eq!(files[0].path, canonical);
    let dropped =
        describe_granted_document_files(&state, vec![path.display().to_string()]).unwrap();
    assert_eq!(dropped[0].name, "memo.pdf");
    assert_eq!(dropped[0].size_bytes, 7);

    fs::remove_file(path).unwrap();
    fs::remove_dir(test_dir).unwrap();
}

#[test]
fn document_job_request_bounds_file_count() {
    let state = AppState::new_for_test().unwrap();
    let error = validate_start_request(
        &state,
        StartDocumentJobsInput {
            paths: (0..=MAX_DOCUMENT_COUNT)
                .map(|index| format!("/tmp/{index}.pdf"))
                .collect(),
            user_id: "user-1".into(),
        },
    )
    .unwrap_err();
    assert_eq!(error, "select no more than 20 documents at once");
}

#[test]
fn search_command_contract_bounds_queries_embeddings_and_limits() {
    assert!(validate_keyword_search(&ChunkKeywordSearch {
        user_id: "user-1".into(),
        query_text: "revenue".into(),
        limit: 10,
    })
    .is_ok());
    assert_eq!(
        validate_keyword_search(&ChunkKeywordSearch {
            user_id: "user-1".into(),
            query_text: " ".into(),
            limit: 10,
        })
        .unwrap_err(),
        "queryText is required"
    );
    assert_eq!(
        validate_vector_search(&ChunkVectorSearch {
            user_id: "user-1".into(),
            query_embedding: vec![0.0; EXPECTED_EMBEDDING_DIMENSION - 1],
            limit: 10,
        })
        .unwrap_err(),
        format!("queryEmbedding must contain {EXPECTED_EMBEDDING_DIMENSION} values")
    );
    assert!(validate_keyword_search(&ChunkKeywordSearch {
        user_id: "user-1".into(),
        query_text: "revenue".into(),
        limit: 101,
    })
    .is_err());
}
