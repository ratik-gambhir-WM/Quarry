use super::*;
fn chunk_with_token_count(token_count: u32) -> DocumentChunk {
    DocumentChunk {
        chunk_id: "chunk-id".to_string(),
        document_id: "document-id".to_string(),
        deal_id: "user-id".to_string(),
        text: "chunk text".to_string(),
        embedding: None,
        sequence_number: 1,
        page_numbers: None,
        start_offset: 0,
        end_offset: 10,
        token_count,
        content_hash: "content-hash".to_string(),
        section_title: None,
    }
}

#[tokio::test]
async fn embedding_attaches_reordered_vectors_to_the_correct_chunks_across_batches() {
    use crate::adapters::postgres::client::PostgresClient;
    use axum::{routing::post, Json, Router};
    use serde_json::{json, Value};
    use std::num::NonZeroUsize;

    for malformed in [false, true] {
        let app = Router::new().route(
            "/embeddings",
            post(move |Json(body): Json<Value>| async move {
                let inputs = body["input"].as_array().unwrap();
                let mut data = inputs
                    .iter()
                    .enumerate()
                    .map(|(index, text)| {
                        let value: f64 = text.as_str().unwrap().parse().unwrap();
                        json!({"index": index, "embedding": [value, -value]})
                    })
                    .rev()
                    .collect::<Vec<_>>();
                if malformed && inputs[0] == "3" {
                    // One valid batch must not cause partial assignment if another
                    // reply contains duplicate indices and omits an input.
                    data[0]["index"] = data[1]["index"].clone();
                }
                Json(json!({"data": data, "usage": {"prompt_tokens": 4, "total_tokens": 4}}))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        let dimensions = NonZeroUsize::new(2).unwrap();
        let client = OpenAiClient::with_embeddings_url(
            format!("http://{address}/embeddings"),
            crate::integration_support::prompt_catalog(),
            dimensions,
        );
        let service = DocumentIngestionService::new(
            DocumentStore::new(PostgresClient::test_database().await.unwrap())
                .with_profile("test".into(), 2),
            super::super::repository::DocumentJobRepository {
                db: PostgresClient::test_database().await.unwrap(),
                profile: crate::domains::documents::store::search_repository::EmbeddingProfile::new(
                    "test".into(),
                    2,
                ),
            },
            None,
            "test".into(),
            "unused".into(),
            1,
        );
        let mut chunks = (1..=4)
            .map(|index| {
                let mut chunk = chunk_with_token_count(150_000);
                chunk.chunk_id = format!("chunk-{index}");
                chunk.text = index.to_string();
                chunk
            })
            .collect::<Vec<_>>();
        assert_eq!(embedding_batches(&chunks).unwrap().len(), 2);
        let result = service
            .embed_chunks("synthetic.txt", 4, &mut chunks, &client)
            .await;
        shutdown.send(()).unwrap();
        server.await.unwrap();
        if malformed {
            assert!(result.is_err());
            assert!(chunks.iter().all(|chunk| chunk.embedding.is_none()));
        } else {
            result.unwrap();
            for (index, chunk) in chunks.iter().enumerate() {
                let value = (index + 1) as f32;
                assert_eq!(chunk.chunk_id, format!("chunk-{}", index + 1));
                assert_eq!(chunk.embedding.as_deref(), Some([value, -value].as_slice()));
            }
        }
    }
}

#[test]
fn embedding_batches_split_at_the_openai_aggregate_token_limit() {
    let mut chunks = vec![chunk_with_token_count(800); 375];
    chunks.push(chunk_with_token_count(1));

    let batches = embedding_batches(&chunks).unwrap();

    assert_eq!(
        batches,
        vec![
            EmbeddingBatch {
                start: 0,
                end: 375,
                token_count: 300_000,
            },
            EmbeddingBatch {
                start: 375,
                end: 376,
                token_count: 1,
            },
        ]
    );
}

#[test]
fn embedding_batches_split_at_the_openai_input_count_limit() {
    let chunks = vec![chunk_with_token_count(1); 2_049];

    let batches = embedding_batches(&chunks).unwrap();

    assert_eq!(
        batches,
        vec![
            EmbeddingBatch {
                start: 0,
                end: 2_048,
                token_count: 2_048,
            },
            EmbeddingBatch {
                start: 2_048,
                end: 2_049,
                token_count: 1,
            },
        ]
    );
}

#[test]
fn embedding_batches_reject_a_chunk_larger_than_one_request() {
    let chunks = vec![chunk_with_token_count(300_001)];

    let error = embedding_batches(&chunks).unwrap_err();

    assert_eq!(
        error,
        "chunk 1 has 300001 tokens, exceeding the 300000-token OpenAI embedding request limit"
    );
}

#[tokio::test]
async fn image_description_requires_a_configured_model_before_calling_openai() {
    let prompts = crate::integration_support::prompt_catalog();
    let client = OpenAiClient::new("test-key", prompts.clone());

    let error = describe_image(b"image bytes", "image/png", &client, "   ")
        .await
        .unwrap_err();

    assert_eq!(error, "image description model cannot be empty");
}

#[tokio::test]
async fn image_uploads_are_validated_before_the_service_requests_a_description() {
    let prompts = crate::integration_support::prompt_catalog();
    let client = OpenAiClient::new("test-key", prompts.clone());
    let result = parse_document(
        UploadedDocument {
            filename: "evidence.png".to_string(),
            bytes: b"not a PNG".to_vec(),
        },
        "user-1".to_string(),
        &client,
        "gpt-image-test",
        Arc::new(Semaphore::new(MAX_CONCURRENT_OFFICE_PARSERS)),
        None,
        None,
    )
    .await;
    let Err(error) = result else {
        panic!("expected invalid image bytes to fail validation");
    };

    assert!(error.starts_with("failed to decode image/png image:"));
}

#[test]
fn upload_content_hash_is_stable_until_document_bytes_change() {
    let original = UploadedDocument {
        filename: "memo.pdf".to_string(),
        bytes: b"same bytes".to_vec(),
    };
    let renamed = UploadedDocument {
        filename: "renamed.pdf".to_string(),
        bytes: b"same bytes".to_vec(),
    };
    let changed = UploadedDocument {
        filename: "memo.pdf".to_string(),
        bytes: b"changed bytes".to_vec(),
    };

    let original_hash = uploaded_document_content_hash(&original);
    assert_eq!(original_hash, uploaded_document_content_hash(&renamed));
    assert_ne!(original_hash, uploaded_document_content_hash(&changed));
}

#[tokio::test]
async fn parsing_keeps_docx_available_when_office_parsing_is_shut_down() {
    use docx_rust::{document::Paragraph, Docx};
    use std::io::Cursor;

    let client = OpenAiClient::new("test-key", crate::integration_support::prompt_catalog());
    let semaphore = Arc::new(Semaphore::new(0));
    semaphore.close();
    for filename in ["evidence.XLSX", "evidence.PpTx"] {
        let result = parse_document(
            UploadedDocument {
                filename: filename.into(),
                bytes: vec![],
            },
            "DEAL-TEST".into(),
            &client,
            "unused",
            semaphore.clone(),
            None,
            None,
        )
        .await;
        assert_eq!(
            result.err().unwrap(),
            "Office document parsing is shutting down"
        );
    }

    let mut source = Docx::default();
    source
        .document
        .push(Paragraph::default().push_text("Synthetic document evidence."));
    let bytes = source.write(Cursor::new(Vec::new())).unwrap().into_inner();
    let result = parse_document(
        UploadedDocument {
            filename: "evidence.docx".into(),
            bytes: bytes.clone(),
        },
        "DEAL-TEST".into(),
        &client,
        "unused",
        semaphore,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.document.file_size_bytes, bytes.len() as u64);
    assert_eq!(result.document.file_name, "evidence.docx");
    assert_eq!(result.document.deal_id, "DEAL-TEST");
    assert!(!result.chunks.is_empty());
}

#[tokio::test]
async fn failed_office_parsing_releases_its_concurrency_permit() {
    let client = OpenAiClient::new("test-key", crate::integration_support::prompt_catalog());
    let semaphore = Arc::new(Semaphore::new(1));
    let result = parse_document(
        UploadedDocument {
            filename: "evidence.xlsx".into(),
            bytes: b"invalid workbook".to_vec(),
        },
        "DEAL-TEST".into(),
        &client,
        "unused",
        semaphore.clone(),
        None,
        None,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(semaphore.available_permits(), 1);
}

#[tokio::test]
async fn upload_stream_accepts_durable_preparation_before_embedding_and_retries_reuse_it() {
    use axum::{routing::post, Json, Router};
    use serde_json::{json, Value};
    use std::{
        io::Cursor,
        sync::atomic::{AtomicUsize, Ordering},
    };
    let visions = Arc::new(AtomicUsize::new(0));
    let embeddings = Arc::new(AtomicUsize::new(0));
    let v = visions.clone();
    let e = embeddings.clone();
    let app=Router::new().route("/responses",post(move||{let v=v.clone();async move{v.fetch_add(1,Ordering::SeqCst);Json(json!({"object":"response","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"A factory floor with three assembly lines."}]}]}))}}))
  .route("/embeddings",post(move|Json(body):Json<Value>|{let e=e.clone();async move{
    let first=e.fetch_add(1,Ordering::SeqCst)==0;
    let vectors=body["input"].as_array().unwrap().iter().enumerate().map(|(index,_)|json!({"index":index,"embedding":if first{vec![0.0;1536]}else{vec![0.5;1536]}})).collect::<Vec<_>>();
    Json(json!({"data":vectors,"usage":{"prompt_tokens":1,"total_tokens":1}}))
  }}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await
            .unwrap();
    });
    let db = crate::domains::documents::test_support::database().await;
    let files = DocumentStore::new(db.clone());
    let queue = super::super::repository::DocumentJobRepository {
        db: db.clone(),
        profile: files.profile().clone(),
    };
    let provider = Arc::new(OpenAiClient::with_embeddings_url(
        format!("http://{address}/embeddings"),
        crate::integration_support::prompt_catalog(),
        std::num::NonZeroUsize::new(1536).unwrap(),
    ));
    let service = DocumentIngestionService::new(
        files,
        queue,
        Some(provider),
        "text-embedding-3-small".into(),
        "test-vision".into(),
        1,
    )
    .with_classification(Arc::new(ClassificationService::new(
        crate::domains::documents::classification::repository::ClassificationRepository::new(
            db.clone(),
        ),
        None,
        crate::integration_support::prompt_catalog(),
        false,
        5,
    )));
    let mut image = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut image, image::ImageOutputFormat::Png)
        .unwrap();
    let bytes = image.into_inner();
    use tower::ServiceExt;
    let service = Arc::new(service);
    let actor = crate::domains::identity::model::AuthenticatedUser {
        user: crate::domains::users::repository::UserRepository::new(db.clone())
            .by_id(1)
            .await
            .unwrap()
            .unwrap(),
        token_hash: "synthetic".into(),
        expires_at: 4102444800,
    };
    let router = super::super::route::routes(
        Arc::clone(&service),
        Arc::new(super::super::job_service::DocumentJobService::new(
            Arc::clone(&service),
        )),
    )
    .layer(axum::Extension(actor));
    let mut body = b"--upload-test\r\nContent-Disposition: form-data; name=\"files\"; filename=\"evidence.png\"\r\nContent-Type: image/png\r\n\r\n".to_vec();
    body.extend_from_slice(&bytes);
    body.extend_from_slice(b"\r\n--upload-test--\r\n");
    let response = router
        .oneshot(
            axum::http::Request::post("/deals/DEAL-SQL/documents/upload")
                .header("content-type", "multipart/form-data; boundary=upload-test")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let frames = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    let frames = std::str::from_utf8(&frames).unwrap();
    assert!(frames.contains("event: accepted"));
    assert!(!frames.contains("embedding"));
    assert_eq!(embeddings.load(Ordering::SeqCst), 0);
    let canonical: i64 = sqlx::query_scalar("SELECT count(*) FROM quarry_files")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(canonical, 0);
    let claim = service.queue().claim_ready().await.unwrap().unwrap();
    assert!(service.process_claim(claim).await.is_err());
    let checkpoint_before: String = sqlx::query_scalar("SELECT prepared_json FROM document_jobs")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let prepared: PreparedDocument = serde_json::from_str(&checkpoint_before).unwrap();
    assert!(matches!(
        prepared.classification.unwrap().outcome,
        crate::domains::documents::classification::repository::ClassificationRun::Disabled
    ));
    assert!(prepared
        .chunks
        .iter()
        .all(|chunk| chunk.embedding.is_none()));
    let second = service
        .process(
            "DEAL-SQL",
            1,
            vec![UploadedDocument {
                filename: "evidence.png".into(),
                bytes: bytes.clone(),
            }],
        )
        .await
        .unwrap();
    let replay = service
        .process(
            "DEAL-SQL",
            1,
            vec![UploadedDocument {
                filename: "evidence.png".into(),
                bytes,
            }],
        )
        .await
        .unwrap();
    stop.send(()).unwrap();
    server.await.unwrap();
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    assert!(second.documents[0].sqlite_committed);
    assert!(second.documents[0].success);
    assert!(replay.documents[0].success);
    assert_eq!(visions.load(Ordering::SeqCst), 1);
    assert_eq!(embeddings.load(Ordering::SeqCst), 2);
    let records:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM quarry_files),(SELECT count(*) FROM deal_activity_events WHERE kind='document_uploaded')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(records, (1, 1));
    let checkpoint_after: String = sqlx::query_scalar("SELECT prepared_json FROM document_jobs")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(checkpoint_before, checkpoint_after);
    let saved_classifications: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM document_classifications WHERE run_status='disabled'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(saved_classifications, 1);
}

#[test]
fn cancelled_parser_keeps_capacity_and_deal_lifetime_until_blocking_work_finishes() {
    use std::{future::Future, task::Poll};
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        // Occupy the only blocking thread so the real parser remains queued deterministically.
        let (release, blocked) = std::sync::mpsc::channel();
        let (started, running) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            blocked.recv().unwrap();
        });
        running.await.unwrap();
        let capacity = Arc::new(Semaphore::new(1));
        let deal = crate::domains::deals::deletion::lifecycle::DealLifecycle::default()
            .admit("DEAL")
            .await;
        let deal_lifetime = Arc::downgrade(&deal);
        let lifetime = AdmissionLifetime {
            _permit: Arc::new(Arc::clone(&capacity).acquire_owned().await.unwrap()),
            _deal: Some(deal),
        };
        let client = OpenAiClient::new("test-key", crate::integration_support::prompt_catalog());
        let mut parsing = Box::pin(parse_document(
            UploadedDocument {
                filename: "invalid.docx".into(),
                bytes: b"not a document".to_vec(),
            },
            "DEAL".into(),
            &client,
            "unused",
            Arc::new(Semaphore::new(1)),
            None,
            Some(lifetime),
        ));
        std::future::poll_fn(|context| {
            assert!(parsing.as_mut().poll(context).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(parsing);
        assert_eq!(capacity.available_permits(), 0);
        assert!(deal_lifetime.upgrade().is_some());
        release.send(()).unwrap();
        blocker.await.unwrap();
        let permit = tokio::time::timeout(std::time::Duration::from_secs(5), capacity.acquire())
            .await
            .unwrap()
            .unwrap();
        assert!(deal_lifetime.upgrade().is_none());
        drop(permit);
        assert_eq!(capacity.available_permits(), 1);
    });
}
