use super::*;

use crate::domains::assistant::chat::repository::AssistantMessage;

struct RevocationServer {
    address: std::net::SocketAddr,
    task: tokio::task::JoinHandle<()>,
}

impl RevocationServer {
    async fn start() -> Self {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"partial\"}\n\n").await.unwrap();
            std::future::pending::<()>().await;
            drop((listener, socket));
        });
        Self { address, task }
    }

    async fn stop(&mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
        assert!(tokio::net::TcpStream::connect(self.address).await.is_err());
    }
}

impl Drop for RevocationServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn assert_chat_revocation(deleting: bool) {
    use crate::{
        adapters::sqlite::client::SqliteClient,
        app::migrations,
        domains::{
            deals::{repository::DealRepository, DealContextReader},
            users::repository::UserRepository,
        },
        integration_support::{prompt_catalog, seed_identity},
    };
    let mut upstream = RevocationServer::start().await;
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrations::migrate(&sqlite).unwrap();
    seed_identity(&sqlite, "owner@example.com", 'a');
    seed_identity(&sqlite, "member@example.com", 'b');
    sqlite.with_connection(|c| c.execute_batch("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES('REVOKE-CHAT',1,'Synthetic','buy_side','unknown','pipeline','now','now'); INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('REVOKE-CHAT',1,'owner','now','now'),('REVOKE-CHAT',2,'viewer','now','now'); INSERT INTO deal_access_revisions(deal_id) VALUES('REVOKE-CHAT');")).unwrap();
    let repository = AssistantChatRepository::new(sqlite.clone());
    repository
        .create_thread(2, "revoked-thread".into(), Some("REVOKE-CHAT".into()))
        .await
        .unwrap();
    let client = OpenAiClient::with_responses_url(
        "synthetic",
        format!("http://{}/v1/responses", upstream.address),
        prompt_catalog(),
    );
    let service = AssistantChatService::with_persistence(
        Some(Arc::new(client)),
        "test".into(),
        repository.clone(),
        UserDirectory::new(UserRepository::new(sqlite.clone())),
        prompt_catalog(),
    )
    .with_deal_context(DealContextReader::new(DealRepository::new(sqlite.clone())));
    let mut events = service
        .ask_with_history(PersistentQueryInput {
            assistant_message_id: "revoked-a".into(),
            user_message_id: "revoked-u".into(),
            user_email: "member@example.com".into(),
            thread_id: "revoked-thread".into(),
            request_id: "revoked-r".into(),
            parent_message_id: None,
            prompt: "synthetic".into(),
            model: None,
            system_instructions: None,
            files: vec![],
            file_references: vec![],
            file_scope_deal_id: None,
        })
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        assert!(matches!(events.recv().await, Some(SendQueryEvent::Started { .. })));
        assert!(matches!(events.recv().await, Some(SendQueryEvent::Delta { delta }) if delta == "partial"));
    }).await.unwrap();
    if deleting {
        sqlite.with_connection(|c| c.execute_batch("INSERT INTO deal_deletions(deletion_id,deal_id,owner_user_id,status,phase,created_at,updated_at) VALUES('revoked-deletion','REVOKE-CHAT',1,'pending','draining','now','now');")).unwrap();
    } else {
        sqlite
            .with_connection(|c| {
                c.execute(
                    "DELETE FROM deal_memberships WHERE deal_id='REVOKE-CHAT' AND user_id=2",
                    [],
                )
            })
            .unwrap();
    }
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        assert!(
            events.recv().await.is_none(),
            "revocation must close the stream"
        );
    })
    .await
    .unwrap();
    upstream.stop().await;
    let (status, content, completed_at): (String, String, Option<String>) = sqlite.with_connection(|c| {
        c.query_row("SELECT status,content,completed_at FROM assistant_messages WHERE message_id='revoked-a'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    }).unwrap();
    if deleting {
        assert_eq!(
            status, "streaming",
            "the deletion barrier must reject late writes"
        );
        assert!(content.is_empty());
        assert!(completed_at.is_none());
    } else {
        assert_eq!(status, "cancelled");
        assert_eq!(content, "partial");
        assert!(completed_at.is_some());
        let thread = service
            .get_thread("member@example.com", "revoked-thread".into())
            .await
            .unwrap();
        assert_eq!(thread.messages.last().unwrap().status, "cancelled");
    }
}

#[tokio::test]
async fn membership_revocation_cancels_the_saved_run_and_preserves_partial_text() {
    assert_chat_revocation(false).await;
}

#[tokio::test]
async fn deletion_revocation_stops_the_run_without_writing_past_the_barrier() {
    assert_chat_revocation(true).await;
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
                repository::{test_support::AddUserInput, UserRepository},
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

            role: "Analyst".into(),
        })
        .await
        .unwrap();
    sqlite.with_connection(|c| c.execute("INSERT INTO deals (deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES ('DEAL-A',?1,'A','buy_side','unknown','pipeline','now','now')", [user.id])).unwrap();
    sqlite.with_connection(|c| c.execute("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('DEAL-A',?1,'owner','now','now')", [user.id])).unwrap();
    sqlite
        .with_connection(|c| {
            c.execute(
                "INSERT INTO deal_access_revisions(deal_id) VALUES('DEAL-A')",
                [],
            )
        })
        .unwrap();
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
        .ask_with_history(PersistentQueryInput {
            file_references: Vec::new(),
            file_scope_deal_id: None,
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

#[tokio::test]
async fn file_references_resolve_canonical_bytes_and_enforce_current_membership() {
    use crate::{
        adapters::{office::converter::OfficeConverter, sqlite::client::SqliteClient},
        app::migrations,
        domains::{
            deals::{repository::DealRepository, DealContextReader},
            documents::{
                store::{
                    identity::IdentityManifest, model::FilePersistenceInput, sqlite::DocumentStore,
                },
                viewing::service::StoredDocumentService,
            },
        },
        shared::ids::{document_id_from_content, file_version_id, sha256_hex},
    };
    let directory = tempfile::tempdir().unwrap();
    let sqlite = SqliteClient::open(directory.path().join("references.sqlite")).unwrap();
    migrations::migrate(&sqlite).unwrap();
    sqlite.with_connection(|c| {
        c.execute("INSERT INTO users (id,first_name,last_name,email,username,password_hash) VALUES (1,'Test','Owner','one@example.com','one','unusable'),(2,'Test','Other','two@example.com','two','unusable')", [])?;
        for deal in ["DEAL-A", "DEAL-B"] {
            c.execute("INSERT INTO deals (deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES (?1,1,'Test','buy_side','unknown','prepare','now','now')", [deal])?;
            c.execute("INSERT INTO deal_memberships (deal_id,user_id,role,joined_at,updated_at) VALUES (?1,1,'owner','now','now')", [deal])?;
            c.execute("INSERT INTO deal_access_revisions (deal_id) VALUES (?1)", [deal])?;
        }
        Ok(())
    }).unwrap();
    let store = DocumentStore::new(sqlite.clone());
    for (deal, file) in [("DEAL-A", "file-a"), ("DEAL-B", "file-b")] {
        let bytes = b"%PDF-1.7\nCanonical stored contents".to_vec();
        let hash = sha256_hex(&bytes);
        let version = file_version_id(file, &hash);
        store
            .persist(FilePersistenceInput {
                deal_id: deal.into(),
                file_id: file.into(),
                actor_id: 1,
                display_name: "Canonical.pdf".into(),
                source_uri: None,
                metadata_json: "{}".into(),
                timestamp: "now".into(),
                version_id: version.clone(),
                mime_type: "application/pdf".into(),
                content_sha256: hash.clone(),
                byte_size: i64::try_from(bytes.len()).unwrap(),
                file_bytes: bytes,
                manifest: Some(IdentityManifest {
                    derivation_version: 2,
                    document_id: document_id_from_content(deal, &hash),
                    active_generation: version,
                    chunks: vec![],
                }),
                image_assembly_json: None,
            })
            .await
            .unwrap();
    }
    let service = AssistantChatService::new(
        None,
        "test".into(),
        crate::integration_support::prompt_catalog(),
    )
    .with_deal_context(DealContextReader::new(DealRepository::new(sqlite.clone())))
    .with_documents(Arc::new(StoredDocumentService::new(
        store,
        OfficeConverter::new(None),
    )));
    let reference =
        |deal: &str, id: &str| crate::domains::assistant::chat::model::ChatFileReference {
            deal_id: deal.into(),
            file_id: id.into(),
            name: "Untrusted client name.pdf".into(),
        };
    let manual = crate::domains::assistant::chat::validate::prepare_attachment(
        "Manual.txt".into(),
        b"Manual text".to_vec(),
        "text/plain".into(),
    )
    .unwrap();
    let resolved = service
        .resolve_file_references(
            1,
            vec![reference("DEAL-A", "file-a"), reference("DEAL-B", "file-b")],
            vec![manual],
        )
        .await
        .unwrap();
    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved[0].filename, "Manual.txt");
    assert_eq!(resolved[1].filename, "Canonical.pdf");
    assert_eq!(resolved[1].bytes, b"%PDF-1.7\nCanonical stored contents");
    assert!(matches!(
        service
            .resolve_file_references(2, vec![reference("DEAL-A", "file-a")], vec![])
            .await,
        Err(ServiceError::NotFound(_))
    ));
    assert!(matches!(
        service
            .resolve_file_references(1, vec![reference("DEAL-A", "file-b")], vec![])
            .await,
        Err(ServiceError::NotFound(_))
    ));
    assert!(matches!(
        service
            .resolve_file_references(1, vec![reference("DEAL-A", "missing")], vec![])
            .await,
        Err(ServiceError::NotFound(_))
    ));
    sqlite
        .with_connection(|c| {
            c.execute(
                "DELETE FROM deal_memberships WHERE deal_id='DEAL-A' AND user_id=1",
                [],
            )
        })
        .unwrap();
    assert!(matches!(
        service
            .resolve_file_references(1, vec![reference("DEAL-A", "file-a")], vec![])
            .await,
        Err(ServiceError::NotFound(_))
    ));
}

fn reference_test_upload(index: usize, size: usize) -> ChatUpload {
    ChatUpload {
        bytes: vec![b'x'; size],
        filename: format!("reference-{index}.txt"),
        kind: ChatUploadKind::File,
        mime_type: "text/plain",
    }
}

#[tokio::test]
async fn file_reference_jobs_overlap_with_a_four_job_bound_and_keep_selection_order() {
    use std::time::Duration;
    use tokio::sync::oneshot;

    let (started_tx, mut started_rx) = mpsc::channel(8);
    let (releases, receivers): (Vec<_>, Vec<_>) = (0..8).map(|_| oneshot::channel()).unzip();
    let mut releases = releases.into_iter().map(Some).collect::<Vec<_>>();
    let resolutions = receivers.into_iter().enumerate().map(|(index, release)| {
        let started = started_tx.clone();
        async move {
            started.send(index).await.unwrap();
            release.await.unwrap();
            Ok(reference_test_upload(index, 1))
        }
    });
    let drive_completions = async {
        let mut first_batch = Vec::new();
        for _ in 0..4 {
            first_batch.push(started_rx.recv().await.unwrap());
        }
        first_batch.sort_unstable();
        assert_eq!(first_batch, [0, 1, 2, 3]);
        assert!(started_rx.try_recv().is_err());
        // Finished jobs free slots even while the first selected file is still pending.
        for release in &mut releases[1..4] {
            release.take().unwrap().send(()).unwrap();
        }
        let mut next_batch = Vec::new();
        for _ in 0..3 {
            next_batch.push(started_rx.recv().await.unwrap());
        }
        next_batch.sort_unstable();
        assert_eq!(next_batch, [4, 5, 6]);
        assert!(started_rx.try_recv().is_err());
        releases[0].take().unwrap().send(()).unwrap();
        assert_eq!(started_rx.recv().await.unwrap(), 7);
        for release in &mut releases[4..] {
            release.take().unwrap().send(()).unwrap();
        }
    };
    let (resolved, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            collect_reference_uploads(vec![reference_test_upload(99, 1)], resolutions),
            drive_completions,
        )
    })
    .await
    .expect("reference jobs must overlap instead of waiting sequentially");
    let filenames = resolved
        .unwrap()
        .into_iter()
        .map(|file| file.filename)
        .collect::<Vec<_>>();
    assert_eq!(
        filenames,
        [99, 0, 1, 2, 3, 4, 5, 6, 7].map(|index| format!("reference-{index}.txt"))
    );
}

#[tokio::test]
async fn file_reference_failure_drops_other_in_flight_async_jobs() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::Notify;

    struct ActiveJob(Arc<AtomicUsize>);
    impl Drop for ActiveJob {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }
    let active = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(Notify::new());
    let (started_tx, mut started_rx) = mpsc::channel(4);
    let resolutions = (0..4).map(|index| {
        let active = active.clone();
        let fail = fail.clone();
        let started = started_tx.clone();
        async move {
            active.fetch_add(1, Ordering::SeqCst);
            let _active_job = ActiveJob(active);
            started.send(index).await.unwrap();
            if index == 0 {
                fail.notified().await;
                Err(ServiceError::not_found("referenced file unavailable"))
            } else {
                std::future::pending::<ServiceResult<ChatUpload>>().await
            }
        }
    });
    let trigger_failure = async {
        for _ in 0..4 {
            started_rx.recv().await.unwrap();
        }
        assert_eq!(active.load(Ordering::SeqCst), 4);
        fail.notify_one();
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            collect_reference_uploads(vec![], resolutions),
            trigger_failure
        )
    })
    .await
    .expect("a failed reference must not wait for unrelated pending jobs");
    assert!(matches!(result, Err(ServiceError::NotFound(_))));
    assert_eq!(active.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn file_reference_collection_preserves_combined_upload_count_and_byte_limits() {
    use crate::shared::file_policy::MAX_TOTAL_REQUEST_FILE_BYTES;
    let uploads = (0..20)
        .map(|index| reference_test_upload(index, 1))
        .collect();
    let result =
        collect_reference_uploads(uploads, [async { Ok(reference_test_upload(20, 1)) }]).await;
    assert!(matches!(result, Err(ServiceError::Validation(_))));

    let result = collect_reference_uploads(
        vec![reference_test_upload(0, MAX_TOTAL_REQUEST_FILE_BYTES)],
        [async { Ok(reference_test_upload(1, 1)) }],
    )
    .await;
    assert!(matches!(result, Err(ServiceError::Validation(_))));

    let result = collect_reference_uploads(
        vec![reference_test_upload(0, MAX_TOTAL_REQUEST_FILE_BYTES - 1)],
        [async { Ok(reference_test_upload(1, 1)) }],
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 2);
}

#[tokio::test]
async fn file_reference_worker_panics_return_a_sanitized_service_error() {
    async fn panicking_reference() -> ServiceResult<ChatUpload> {
        panic!("internal worker panic details");
    }
    let result = collect_reference_uploads(vec![], [panicking_reference()]).await;
    assert_eq!(
        result.unwrap_err(),
        ServiceError::internal("file reference task failed")
    );
}
