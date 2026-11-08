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
fn build_graph_rag_query_request_body_keeps_application_name_server_side() {
    let payload = GraphRagQueryPayload {
        resource_id: "resource-123".to_string(),
        question: "What are the risks?".to_string(),
    };

    assert_eq!(
        build_graph_rag_query_request_body("quarry-dev", &payload),
        json!({
            "applicationName": "quarry-dev",
            "question": "What are the risks?",
        })
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

#[test]
fn resource_url_encodes_identifiers_without_changing_the_upstream_base_path() {
    let url = resource_url(
        "https://services.example.test/dev",
        &["indexes", "id?next=other#fragment%2F", "status"],
    )
    .unwrap();
    assert_eq!(
        url.as_str(),
        "https://services.example.test/dev/indexes/id%3Fnext=other%23fragment%252F/status"
    );
    assert_eq!(url.query(), None);
    assert_eq!(url.fragment(), None);
}
