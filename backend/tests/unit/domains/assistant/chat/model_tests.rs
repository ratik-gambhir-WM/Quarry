use super::*;

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
