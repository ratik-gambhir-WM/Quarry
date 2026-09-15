use super::*;

#[test]
fn unavailable_capability_fails_before_streaming() {
    let service = AssistantChatService::new(None, "gpt-5.5".to_string());
    let result = service.ask(QueryModelInput {
        context: Vec::new(),
        files: Vec::new(),
        model: None,
        prompt: "hello".to_string(),
        system_instructions: None,
    });
    assert!(matches!(result, Err(ServiceError::Unavailable(_))));
}
