use super::*;

use crate::domains::assistant::chat::repository::AssistantMessage;

#[test]
fn unavailable_capability_fails_before_streaming() {
    let service = AssistantChatService::new(
        None,
        "gpt-5.5".to_string(),
        crate::integration_support::prompt_catalog(),
    );
    let result = service.ask(QueryModelInput {
        context: Vec::new(),
        files: Vec::new(),
        model: None,
        prompt: "hello".to_string(),
        system_instructions: None,
    });
    assert!(matches!(result, Err(ServiceError::Unavailable(_))));
}

#[test]
fn uses_the_catalog_default_when_the_client_omits_system_instructions() {
    let prompts = crate::integration_support::prompt_catalog();

    assert_eq!(
        system_instructions_or_default(None, prompts.as_ref()),
        prompts.prompt(PromptId::AssistantChatDefault)
    );
}

#[test]
fn preserves_client_system_instructions_for_future_custom_prompts() {
    let prompts = crate::integration_support::prompt_catalog();

    assert_eq!(
        system_instructions_or_default(Some("Be concise.".to_string()), prompts.as_ref()),
        "Be concise."
    );
}

#[test]
fn persisted_context_follows_the_retried_response_branch() {
    let context = bounded_provider_context(
        vec![
            message("user-1", None, 0, "user", "Question"),
            message(
                "assistant-1",
                Some("user-1"),
                1,
                "assistant",
                "First answer",
            ),
            message(
                "assistant-2",
                Some("user-1"),
                2,
                "assistant",
                "Retried answer",
            ),
            message("user-2", Some("assistant-2"), 3, "user", "Follow-up"),
            message(
                "assistant-3",
                Some("user-2"),
                4,
                "assistant",
                "Follow-up answer",
            ),
        ],
        Some("assistant-3"),
    );

    assert_eq!(context.len(), 4);
    assert_eq!(context[0].content, "Question");
    assert_eq!(context[1].content, "Retried answer");
    assert_eq!(context[2].content, "Follow-up");
    assert_eq!(context[3].content, "Follow-up answer");
}

fn message(
    message_id: &str,
    parent_message_id: Option<&str>,
    sequence: i64,
    role: &str,
    content: &str,
) -> AssistantMessage {
    AssistantMessage {
        message_id: message_id.to_string(),
        thread_id: "thread-1".to_string(),
        parent_message_id: parent_message_id.map(str::to_string),
        sequence,
        role: role.to_string(),
        content: content.to_string(),
        status: "completed".to_string(),
        model: None,
        created_at: "2026-09-21 00:00:00".to_string(),
        completed_at: Some("2026-09-21 00:00:01".to_string()),
    }
}
