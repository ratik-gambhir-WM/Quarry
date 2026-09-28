use super::*;

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
