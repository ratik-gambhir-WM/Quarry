use super::*;

#[test]
fn service_url_preserves_existing_base_path() {
    let url = service_url("https://services.example.test/dev", "indexes").unwrap();

    assert_eq!(url.as_str(), "https://services.example.test/dev/indexes");
}

#[test]
fn build_create_index_request_body_uses_extracted_documents() {
    let payload = CreateIndexPayload {
        name: Some("Deal Room".to_string()),
        files: vec![IndexDocumentInput {
            file_id: Some("file-123".to_string()),
            filename: "memo.pdf".to_string(),
            text: "Important details".to_string(),
            metadata: json!({ "pageCount": 2 }),
        }],
    };

    assert_eq!(
        build_create_index_request_body(&payload),
        json!({
            "name": "Deal Room",
            "documents": [
                {
                    "fileId": "file-123",
                    "filename": "memo.pdf",
                    "text": "Important details",
                    "metadata": { "pageCount": 2 },
                }
            ],
        })
    );
}

#[test]
fn validate_create_index_payload_rejects_empty_text() {
    let payload = CreateIndexPayload {
        name: None,
        files: vec![IndexDocumentInput {
            file_id: None,
            filename: "memo.pdf".to_string(),
            text: " ".to_string(),
            metadata: Value::Null,
        }],
    };

    assert_eq!(
        validate_create_index_payload(&payload).unwrap_err(),
        "extracted text cannot be empty for memo.pdf"
    );
}

#[test]
fn build_graph_rag_query_request_body_keeps_application_name_server_side() {
    let payload = GraphRagQueryPayload {
        resource_id: "resource-123".to_string(),
        question: "What are the risks?".to_string(),
    };

    assert_eq!(
        build_graph_rag_query_request_body("pathfinder-dev", &payload),
        json!({
            "applicationName": "pathfinder-dev",
            "question": "What are the risks?",
        })
    );
}

#[test]
fn validate_graph_rag_query_payload_rejects_empty_question() {
    let payload = GraphRagQueryPayload {
        resource_id: "resource-123".to_string(),
        question: " ".to_string(),
    };

    assert_eq!(
        validate_graph_rag_query_payload(&payload).unwrap_err(),
        "question cannot be empty"
    );
}

#[test]
fn index_status_response_accepts_capitalized_status_from_service() {
    let parsed: IndexStatusResponse =
        serde_json::from_value(json!({ "Status": "Ready", "resourceId": "graph-123" })).unwrap();

    assert_eq!(
        parsed,
        IndexStatusResponse {
            status: "Ready".to_string(),
            resource_id: Some("graph-123".to_string()),
        }
    );
}
