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
    let client = OpenAiClient::new(reqwest::Client::new(), "test-key", prompts.clone());

    let error = describe_image(b"image bytes", "image/png", &client, "   ")
        .await
        .unwrap_err();

    assert_eq!(error, "image description model cannot be empty");
}

#[tokio::test]
async fn image_uploads_are_validated_before_the_service_requests_a_description() {
    let prompts = crate::integration_support::prompt_catalog();
    let client = OpenAiClient::new(reqwest::Client::new(), "test-key", prompts.clone());
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

#[test]
fn shared_upload_errors_disclose_only_recovery_state() {
    let failed = failed_document(
        "synthetic.pdf".into(),
        "provider failure containing secret details".into(),
    );
    assert!(!failed.sqlite_committed);
    assert!(!failed.error.unwrap().contains("secret"));
    let committed=failed_document("synthetic.pdf".into(),"Helix indexing failed after SQLite committed file_id 'synthetic' and version_id 'synthetic': secret details".into());
    assert!(committed.sqlite_committed);
    let error = committed.error.unwrap();
    assert!(error.contains("stored"));
    assert!(error.contains("retry the same file"));
    assert!(!error.contains("secret"));
}

#[tokio::test]
async fn image_recovery_reuses_accepted_text_after_graph_failure_and_sqlite_reopen() {
    use crate::{
        adapters::{helix::client::HelixClient, sqlite::client::SqliteClient},
        app::migrations::migrate,
    };
    use axum::{routing::post, Json, Router};
    use serde_json::{json, Value};
    use std::io::Cursor;

    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let captured = Arc::clone(&requests);
    let app = Router::new().route(
        "/v2/query",
        post(move |Json(body): Json<Value>| {
            let captured = Arc::clone(&captured);
            async move {
                let mut requests = captured.lock().await;
                requests.push(body);
                if requests.len() == 1 {
                    (
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        Json(json!({"error":"unavailable"})),
                    )
                } else {
                    (axum::http::StatusCode::OK, Json(json!({})))
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let writer = DocumentGraphWriter::new(Arc::new(
        HelixClient::with_config(&format!("http://{address}"), None).unwrap(),
    ));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image-recovery.sqlite");
    let sqlite = SqliteClient::open(&path).unwrap();
    migrate(&sqlite).unwrap();
    crate::integration_support::seed_identity(&sqlite, "image-test@example.com", 'i');
    sqlite.with_connection(|c| {
        c.execute_batch("INSERT INTO deals(deal_id,user_id,deal_name,status,created_at,updated_at,engagement_type,transaction_type) VALUES('DEAL-IMAGE',1,'Image recovery','prepare','now','now','buy_side','unknown');
        INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('DEAL-IMAGE',1,'owner','now','now');
        INSERT INTO deal_access_revisions(deal_id) VALUES('DEAL-IMAGE');")
    }).unwrap();
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut bytes, image::ImageOutputFormat::Png)
        .unwrap();
    let bytes = bytes.into_inner();
    let accepted_text = "A factory floor with three assembly lines.";
    let mut accepted = build_image_assembly(
        bytes.clone(),
        None,
        "evidence.png".into(),
        "DEAL-IMAGE",
        accepted_text,
    )
    .unwrap();
    for chunk in &mut accepted.chunks {
        chunk.embedding = Some(vec![0.25, 0.5]);
    }
    let version = crate::shared::ids::file_version_id(
        &accepted.document.file_id,
        &accepted.document.content_hash,
    );
    let files = DocumentStore::new(sqlite.clone());
    let failure = persist_document_and_chunks(
        &files,
        &writer,
        "DEAL-IMAGE",
        1,
        accepted.document.clone(),
        accepted.chunks.clone(),
        bytes.clone(),
    )
    .await
    .unwrap_err();
    assert!(failure.contains("Helix indexing failed after SQLite committed"));
    let manifest = files.manifest(version.clone()).await.unwrap().unwrap();
    drop(files);
    drop(sqlite);

    let sqlite = SqliteClient::open(&path).unwrap();
    migrate(&sqlite).unwrap();
    let files = DocumentStore::new(sqlite);
    let recovery = files.image_assembly(&version).await.unwrap().unwrap();
    assert!(recovery.chunks.iter().all(|c| c.embedding.is_none()));
    let mut alternative = build_image_assembly(
        bytes.clone(),
        None,
        "evidence.png".into(),
        "DEAL-IMAGE",
        "Three assembly lines are visible inside a factory.",
    )
    .unwrap();
    assert_ne!(
        accepted.chunks[0].content_hash,
        alternative.chunks[0].content_hash
    );
    alternative
        .document
        .file_id
        .clone_from(&accepted.document.file_id);
    for chunk in &mut alternative.chunks {
        chunk.embedding = Some(vec![0.25, 0.5]);
    }
    let changed = persist_document_and_chunks(
        &files,
        &writer,
        "DEAL-IMAGE",
        1,
        alternative.document,
        alternative.chunks,
        bytes.clone(),
    )
    .await
    .unwrap_err();
    assert!(changed.contains("parser chunk differs from immutable identity manifest"));
    let client = OpenAiClient::new(
        reqwest::Client::new(),
        "unused-test-key",
        crate::integration_support::prompt_catalog(),
    );
    // An empty model would fail before any vision request if recovery tried describing again.
    let mut retry = parse_document(
        UploadedDocument {
            filename: "renamed.png".into(),
            bytes,
        },
        "DEAL-IMAGE".into(),
        &client,
        "",
        Arc::new(Semaphore::new(2)),
        Some(recovery),
    )
    .await
    .unwrap();
    assert_eq!(retry.chunks[0].text, accepted_text);
    for chunk in &mut retry.chunks {
        chunk.embedding = Some(vec![0.75, 0.5]);
    }
    persist_document_and_chunks(
        &files,
        &writer,
        "DEAL-IMAGE",
        1,
        retry.document,
        retry.chunks,
        retry.file_bytes,
    )
    .await
    .unwrap();
    assert_eq!(
        files.manifest(version.clone()).await.unwrap().unwrap(),
        manifest
    );
    assert_eq!(
        files
            .image_assembly(&version)
            .await
            .unwrap()
            .unwrap()
            .chunks[0]
            .text,
        accepted_text
    );
    let requests = requests.lock().await;
    assert_eq!(requests.len(), 4);
    assert!(requests[2].to_string().contains(accepted_text));
    server.abort();
    let _ = server.await;
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
}
