use super::*;

#[tokio::test]
async fn persists_file_bytes_under_the_user_scoped_content_id() {
    let state = crate::state::AppState::in_memory().unwrap();
    let file_bytes = b"binary\0file\xffbytes".to_vec();
    let content_hash = sha256_hex(&file_bytes);
    let file_id = document_id_from_content("user-1", &content_hash);
    let document = DocumentNode {
        document_id: file_id.clone(),
        user_id: "user-1".to_string(),
        file_name: "test.pdf".to_string(),
        source_type: "pdf".to_string(),
        local_path: None,
        file_size_bytes: file_bytes.len() as u64,
        token_count: 0,
        content_hash,
        rendered_pdf_path: None,
    };

    let persisted_file_id = persist_file_blob(state.sqlite(), &document, file_bytes.clone())
        .await
        .unwrap();
    let query = SqlBuilder::select("quarry_file_blobs")
        .column("file_bytes")
        .and_where(crate::core::sqlbuilder::Condition::equal(
            "file_id", &file_id,
        ))
        .build()
        .unwrap();
    let stored = state.sqlite().read_one(&query).unwrap().unwrap();

    assert_eq!(persisted_file_id, file_id);
    assert_eq!(
        stored.get("file_bytes"),
        Some(&crate::core::sqlbuilder::SqlValue::Blob(file_bytes))
    );
}

#[test]
fn deserializes_a_quarry_file_lookup() {
    let response: QuarryFileLookupResponse = serde_json::from_value(serde_json::json!({
        "quarry_file": {
            "properties": [{ "document_id": "document-1" }]
        }
    }))
    .unwrap();

    assert_eq!(response.quarry_file.properties[0].document_id, "document-1");
}

#[test]
fn deserializes_an_empty_quarry_file_lookup() {
    let response: QuarryFileLookupResponse = serde_json::from_value(serde_json::json!({
        "quarry_file": { "properties": [] }
    }))
    .unwrap();

    assert!(response.quarry_file.properties.is_empty());
}

#[test]
fn rejects_a_chunk_for_another_document_before_persisting() {
    let document = DocumentNode {
        document_id: "doc-1".to_string(),
        user_id: "user-1".to_string(),
        file_name: "test.pdf".to_string(),
        source_type: "pdf".to_string(),
        local_path: None,
        file_size_bytes: 1,
        token_count: 1,
        content_hash: "hash".to_string(),
        rendered_pdf_path: None,
    };
    let chunk = ChunkNode {
        chunk_id: "chunk-1".to_string(),
        document_id: "doc-2".to_string(),
        user_id: "user-1".to_string(),
        text: "text".to_string(),
        embedding: Some(vec![1.0]),
        sequence_number: 1,
        page_numbers: None,
        start_offset: 0,
        end_offset: 4,
        token_count: 1,
        content_hash: "hash".to_string(),
        section_title: None,
    };

    assert!(validate_document_chunk_relationship(&document, &chunk).is_err());
}
