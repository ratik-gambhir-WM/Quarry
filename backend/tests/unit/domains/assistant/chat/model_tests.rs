use super::*;

#[test]
fn context_requires_complete_alternating_pairs_and_preserves_text() {
    let value =
        r#"[{"role":"user","content":"  question  "},{"role":"assistant","content":"answer"}]"#;
    let context = parse_context(value).unwrap();
    assert_eq!(context[0].content, "  question  ");

    assert!(parse_context(r#"[{"role":"assistant","content":"answer"}]"#).is_err());
    assert!(parse_context(r#"[{"role":"user","content":"question"}]"#).is_err());
    assert!(parse_context(
        r#"[{"role":"user","content":"q","extra":true},{"role":"assistant","content":"a"}]"#
    )
    .is_err());
}

#[test]
fn scalar_validation_preserves_text_but_normalizes_model() {
    assert_eq!(
        validate_prompt("  prompt  ".to_string()).unwrap(),
        "  prompt  "
    );
    assert_eq!(
        validate_model(Some("  gpt-5.5  ".to_string()))
            .unwrap()
            .as_deref(),
        Some("gpt-5.5")
    );
    assert!(validate_model(Some(" \n ".to_string())).is_err());
}

#[test]
fn persisted_started_event_uses_the_camel_case_sse_contract() {
    let event = SendQueryEvent::Started {
        model: "gpt-5.5".to_string(),
        thread_id: Some("thread-1".to_string()),
        user_message_id: Some("user-1".to_string()),
        assistant_message_id: Some("assistant-1".to_string()),
    };

    assert_eq!(
        serde_json::to_value(event).unwrap(),
        serde_json::json!({
            "type": "started",
            "model": "gpt-5.5",
            "threadId": "thread-1",
            "userMessageId": "user-1",
            "assistantMessageId": "assistant-1",
        })
    );
}
