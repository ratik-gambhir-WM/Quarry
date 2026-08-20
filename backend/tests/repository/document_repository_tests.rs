use super::*;

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
