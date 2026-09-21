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
fn parser_rejects_a_terminated_oversized_event() {
    let mut parser = QueryRelayParser::default();
    let event = format!(
        "event: started\ndata: {{\"type\":\"started\",\"model\":\"{}\"}}\n\n",
        "x".repeat(MAX_EVENT_BYTES)
    );

    assert_eq!(
        parser.push(event.as_bytes()).unwrap_err(),
        "Quarry query stream event is too large"
    );
}

#[test]
fn query_request_path_is_restricted_to_the_two_assistant_contracts() {
    assert!(validate_query_path("/api/v1/query_model").is_ok());
    assert!(validate_query_path("/api/v1/assistant/threads/thread-1/runs").is_ok());
    assert!(validate_query_path("https://example.com").is_err());
    assert!(validate_query_path("/api/v1/assistant/threads/thread-1/runs/extra").is_err());
}
