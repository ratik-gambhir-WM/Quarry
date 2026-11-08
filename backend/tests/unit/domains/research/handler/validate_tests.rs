use super::*;
use serde_json::json;

#[test]
fn validate_create_index_payload_rejects_empty_text() {
    let payload: CreateIndexPayload =
        serde_json::from_value(json!({"files":[{"filename":"memo.pdf","text":" "}]})).unwrap();

    assert_eq!(
        validate_create_index_payload(&payload)
            .unwrap_err()
            .to_string(),
        "extracted text cannot be empty for memo.pdf"
    );
}

#[test]
fn validate_graph_rag_query_payload_rejects_empty_question() {
    let payload = GraphRagQueryPayload {
        resource_id: "resource-123".to_string(),
        question: " ".to_string(),
    };

    assert_eq!(
        validate_graph_rag_query_payload(&payload)
            .unwrap_err()
            .to_string(),
        "question cannot be empty"
    );
}
