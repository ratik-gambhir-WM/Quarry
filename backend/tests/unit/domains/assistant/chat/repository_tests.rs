use crate::{
    adapters::sqlite::client::SqliteClient,
    app::migrations,
    domains::{
        assistant::chat::repository::{AssistantChatRepository, StartRunInput, StartRunResult},
        users::repository::{AddUserInput, UserRepository},
    },
};

async fn repository_fixture() -> (AssistantChatRepository, i64, SqliteClient) {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrations::migrate(&sqlite).unwrap();
    let user = UserRepository::new(sqlite.clone())
        .create(AddUserInput {
            api_key: "development-key".to_string(),
            email: "analyst@example.com".to_string(),
            first_name: "Avery".to_string(),
            last_name: "Analyst".to_string(),
            role: "Analyst".to_string(),
        })
        .await
        .unwrap();
    (
        AssistantChatRepository::new(sqlite.clone()),
        user.id,
        sqlite,
    )
}

#[tokio::test]
async fn run_persists_an_ordered_pair_and_replays_the_same_request_id() {
    let (repository, user_id, _) = repository_fixture().await;
    repository
        .create_thread(user_id, "thread-1".to_string())
        .await
        .unwrap();
    let input = StartRunInput {
        assistant_message_id: "assistant-1".to_string(),
        model: "gpt-test".to_string(),
        parent_message_id: None,
        prompt: "Review the deal".to_string(),
        request_id: "request-1".to_string(),
        thread_id: "thread-1".to_string(),
        user_id,
        user_message_id: "user-1".to_string(),
    };

    assert!(matches!(
        repository.start_run(input.clone()).await.unwrap(),
        StartRunResult::Started { prior_messages } if prior_messages.is_empty()
    ));
    repository
        .finish_run(
            "assistant-1".to_string(),
            "completed",
            "The deal is ready.".to_string(),
            None,
        )
        .await
        .unwrap();
    assert!(matches!(
        repository.start_run(input).await.unwrap(),
        StartRunResult::Existing(message)
            if message.status == "completed" && message.content == "The deal is ready."
    ));

    let thread = repository
        .get_thread(user_id, "thread-1".to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(thread.thread.title, "Review the deal");
    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[0].sequence, 0);
    assert_eq!(thread.messages[1].sequence, 1);
}

#[tokio::test]
async fn retry_reuses_the_user_message_and_excludes_the_retried_pair_from_context() {
    let (repository, user_id, sqlite) = repository_fixture().await;
    repository
        .create_thread(user_id, "thread-1".to_string())
        .await
        .unwrap();
    let first = StartRunInput {
        assistant_message_id: "assistant-1".to_string(),
        model: "gpt-test".to_string(),
        parent_message_id: None,
        prompt: "Review the deal".to_string(),
        request_id: "request-1".to_string(),
        thread_id: "thread-1".to_string(),
        user_id,
        user_message_id: "user-1".to_string(),
    };
    repository.start_run(first).await.unwrap();
    repository
        .finish_run(
            "assistant-1".to_string(),
            "completed",
            "First answer".to_string(),
            None,
        )
        .await
        .unwrap();
    sqlite
        .with_connection(|connection| {
            connection.execute(
                "UPDATE assistant_messages SET parent_message_id = message_id WHERE message_id = 'user-1'",
                [],
            )?;
            Ok(())
        })
        .unwrap();

    let retry = StartRunInput {
        assistant_message_id: "assistant-2".to_string(),
        model: "gpt-test".to_string(),
        parent_message_id: None,
        prompt: "Review the deal".to_string(),
        request_id: "request-2".to_string(),
        thread_id: "thread-1".to_string(),
        user_id,
        user_message_id: "user-1".to_string(),
    };
    assert!(matches!(
        repository.start_run(retry).await.unwrap(),
        StartRunResult::Started { prior_messages } if prior_messages.is_empty()
    ));

    let thread = repository
        .get_thread(user_id, "thread-1".to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(thread.messages.len(), 3);
    assert_eq!(thread.messages[0].message_id, "user-1");
    assert_eq!(thread.messages[0].parent_message_id, None);
    assert_eq!(thread.messages[1].message_id, "assistant-1");
    assert_eq!(thread.messages[2].message_id, "assistant-2");
    assert_eq!(thread.messages[2].sequence, 2);
    assert_eq!(
        thread.messages[2].parent_message_id.as_deref(),
        Some("user-1")
    );
}

#[tokio::test]
async fn run_rejects_reused_message_ids_that_do_not_describe_a_retry() {
    let (repository, user_id, _) = repository_fixture().await;
    repository
        .create_thread(user_id, "thread-1".to_string())
        .await
        .unwrap();
    repository
        .start_run(StartRunInput {
            assistant_message_id: "assistant-1".to_string(),
            model: "gpt-test".to_string(),
            parent_message_id: None,
            prompt: "Original prompt".to_string(),
            request_id: "request-1".to_string(),
            thread_id: "thread-1".to_string(),
            user_id,
            user_message_id: "user-1".to_string(),
        })
        .await
        .unwrap();

    let conflicting_user = StartRunInput {
        assistant_message_id: "assistant-2".to_string(),
        model: "gpt-test".to_string(),
        parent_message_id: None,
        prompt: "Different prompt".to_string(),
        request_id: "request-2".to_string(),
        thread_id: "thread-1".to_string(),
        user_id,
        user_message_id: "user-1".to_string(),
    };
    assert!(matches!(
        repository.start_run(conflicting_user).await.unwrap(),
        StartRunResult::Conflict
    ));

    let conflicting_assistant = StartRunInput {
        assistant_message_id: "assistant-1".to_string(),
        model: "gpt-test".to_string(),
        parent_message_id: None,
        prompt: "Another prompt".to_string(),
        request_id: "request-3".to_string(),
        thread_id: "thread-1".to_string(),
        user_id,
        user_message_id: "user-3".to_string(),
    };
    assert!(matches!(
        repository.start_run(conflicting_assistant).await.unwrap(),
        StartRunResult::Conflict
    ));

    let identical_pair_ids = StartRunInput {
        assistant_message_id: "message-4".to_string(),
        model: "gpt-test".to_string(),
        parent_message_id: Some("assistant-1".to_string()),
        prompt: "Follow up".to_string(),
        request_id: "request-4".to_string(),
        thread_id: "thread-1".to_string(),
        user_id,
        user_message_id: "message-4".to_string(),
    };
    assert!(matches!(
        repository.start_run(identical_pair_ids).await.unwrap(),
        StartRunResult::Conflict
    ));

    let thread = repository
        .get_thread(user_id, "thread-1".to_string())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(thread.messages.len(), 2);
}

#[tokio::test]
async fn ownership_and_archive_filtering_are_enforced() {
    let (repository, user_id, _) = repository_fixture().await;
    repository
        .create_thread(user_id, "thread-1".to_string())
        .await
        .unwrap();

    assert!(repository
        .get_thread(user_id + 1, "thread-1".to_string())
        .await
        .unwrap()
        .is_none());
    assert!(repository
        .set_thread_status(user_id, "thread-1".to_string(), "archived".to_string())
        .await
        .unwrap());
    assert!(repository
        .list_threads(user_id, "regular".to_string(), 10, None)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        repository
            .list_threads(user_id, "archived".to_string(), 10, None)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn thread_pagination_keeps_same_timestamp_ties() {
    let (repository, user_id, sqlite) = repository_fixture().await;
    repository
        .create_thread(user_id, "thread-a".to_string())
        .await
        .unwrap();
    repository
        .create_thread(user_id, "thread-b".to_string())
        .await
        .unwrap();
    sqlite
        .with_connection(|connection| {
            connection.execute(
                "UPDATE assistant_threads SET last_message_at = '2026-01-01 00:00:00' WHERE user_id = ?1",
                [user_id],
            )?;
            Ok(())
        })
        .unwrap();

    let first_page = repository
        .list_threads(user_id, "regular".to_string(), 1, None)
        .await
        .unwrap();
    let first = &first_page[0];
    let cursor = format!("{}|{}", first.last_message_at, first.thread_id);
    let second_page = repository
        .list_threads(user_id, "regular".to_string(), 1, Some(cursor))
        .await
        .unwrap();

    assert_eq!(second_page.len(), 1);
    assert_ne!(second_page[0].thread_id, first.thread_id);
}
