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

#[tokio::test]
async fn completed_request_replays_without_provider_or_current_deal_context() {
    use crate::{
        adapters::sqlite::client::SqliteClient,
        app::migrations,
        domains::{
            assistant::chat::repository::{AssistantChatRepository, StartRunInput},
            users::{
                repository::{AddUserInput, UserRepository},
                UserDirectory,
            },
        },
    };
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrations::migrate(&sqlite).unwrap();
    let users = UserRepository::new(sqlite.clone());
    let user = users
        .create(AddUserInput {
            first_name: "Test".into(),
            last_name: "Owner".into(),
            email: "owner@example.com".into(),
            api_key: "unused".into(),
            role: "Analyst".into(),
        })
        .await
        .unwrap();
    sqlite.with_connection(|c| c.execute("INSERT INTO deals (deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES ('DEAL-A',?1,'A','buy_side','unknown','pipeline','now','now')", [user.id])).unwrap();
    let repository = AssistantChatRepository::new(sqlite.clone());
    repository
        .create_thread(user.id, "thread-a".into(), Some("DEAL-A".into()))
        .await
        .unwrap();
    repository
        .start_run(StartRunInput {
            assistant_message_id: "a1".into(),
            user_message_id: "u1".into(),
            model: "test".into(),
            parent_message_id: None,
            prompt: "Question".into(),
            request_id: "r1".into(),
            thread_id: "thread-a".into(),
            user_id: user.id,
        })
        .await
        .unwrap();
    repository
        .finish_run("a1".into(), "completed", "Persisted answer".into(), None)
        .await
        .unwrap();
    sqlite
        .with_connection(|c| {
            c.execute(
                "UPDATE deals SET deal_name='Changed', archived_at='now'",
                [],
            )
        })
        .unwrap();
    let service = AssistantChatService::with_persistence(
        None,
        "test".into(),
        repository,
        UserDirectory::new(users),
        crate::integration_support::prompt_catalog(),
    );
    let mut events = service
        .ask_persisted(PersistentQueryInput {
            assistant_message_id: "a1".into(),
            user_message_id: "u1".into(),
            model: None,
            parent_message_id: None,
            prompt: "Question".into(),
            request_id: "r1".into(),
            thread_id: "thread-a".into(),
            user_email: "owner@example.com".into(),
            files: vec![],
            system_instructions: None,
        })
        .await
        .unwrap();
    assert!(matches!(
        events.recv().await,
        Some(SendQueryEvent::Started { .. })
    ));
    assert!(
        matches!(events.recv().await,Some(SendQueryEvent::Completed { response }) if response == "Persisted answer")
    );
}
