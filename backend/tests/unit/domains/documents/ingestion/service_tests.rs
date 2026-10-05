use super::*;

fn chunk_with_token_count(token_count: u32) -> DocumentChunk {
    DocumentChunk {
        chunk_id: "chunk-id".to_string(),
        document_id: "document-id".to_string(),
        user_id: "user-id".to_string(),
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
    let client = OpenAiClient::new(reqwest::Client::new(), "test-key");

    let error = describe_image(b"image bytes", "image/png", &client, "   ")
        .await
        .unwrap_err();

    assert_eq!(error, "image description model cannot be empty");
}

#[tokio::test]
async fn image_uploads_are_validated_before_the_service_requests_a_description() {
    let client = OpenAiClient::new(reqwest::Client::new(), "test-key");
    let result = parse_document(
        UploadedDocument {
            filename: "evidence.png".to_string(),
            bytes: b"not a PNG".to_vec(),
        },
        "user-1".to_string(),
        &client,
        "gpt-image-test",
        Arc::new(Semaphore::new(MAX_CONCURRENT_OFFICE_PARSERS)),
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
