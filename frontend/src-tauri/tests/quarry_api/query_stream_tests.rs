use super::*;

#[test]
fn validates_context_pairs_and_preserves_message_text() {
    let valid = [
        super::super::models::ChatContextMessage {
            content: "question".to_string(),
            role: ChatContextRole::User,
        },
        super::super::models::ChatContextMessage {
            content: "  answer  ".to_string(),
            role: ChatContextRole::Assistant,
        },
    ];
    assert!(validate_context(&valid).is_ok());
    assert!(validate_context(&valid[..1]).is_err());
}

#[test]
fn parser_requires_started_and_one_terminal() {
    let mut parser = QueryRelayParser::default();
    let events = parser.push(
        b"event: started\r\ndata: {\"type\":\"started\",\"model\":\"gpt-5.5\"}\r\n\r\nevent: delta\ndata: {\"type\":\"delta\",\"delta\":\"hi\"}\n\nevent: completed\ndata: {\"type\":\"completed\",\"response\":\"hello\"}\n\n",
    ).unwrap();
    assert_eq!(events.len(), 3);
    assert!(parser.finish().is_ok());

    let mut missing_start = QueryRelayParser::default();
    assert!(missing_start
        .push(b"event: completed\ndata: {\"type\":\"completed\",\"response\":\"x\"}\n\n")
        .is_err());
}

#[test]
fn query_request_has_no_caller_controlled_path() {
    let value = serde_json::json!({
        "context": [],
        "files": [],
        "path": "https://example.com",
        "prompt": "hello"
    });
    assert!(serde_json::from_value::<QueryStreamRequest>(value).is_err());
}
