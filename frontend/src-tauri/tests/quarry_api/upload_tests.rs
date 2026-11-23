use super::*;

#[tokio::test]
async fn upload_rejects_external_paths_before_preparing_file_bytes() {
    let service = QuarryApiService::from_environment().unwrap();
    for path in [
        r"https:\example.invalid/api/v1/deals/DEAL/documents/upload",
        "https://example.invalid/api/v1/deals/DEAL/documents/upload",
        "//example.invalid/api/v1/deals/DEAL/documents/upload",
        "relative/api/v1/deals/DEAL/documents/upload",
    ] {
        let error = service
            .upload_document(
                MultipartRequest {
                    path: path.into(),
                    fields: vec![],
                    files: vec![super::super::models::MultipartFile {
                        field_name: "files".into(),
                        filename: "example.pdf".into(),
                        mime_type: "application/pdf".into(),
                        // A distinct multipart error would reveal that path validation
                        // happened after byte preparation. No network request can run.
                        data_base64: String::new(),
                    }],
                },
                "upload-test",
                |_| panic!("invalid uploads must not emit progress"),
            )
            .await
            .unwrap_err();
        assert_eq!(error, "Quarry API path is not allowed");
    }
}

#[test]
fn upload_progress_rejects_embedding_and_malformed_events() {
    assert!(parse_progress(r#"{"jobId":"job","filename":"a.pdf","status":"accepted"}"#).is_ok());
    for status in ["embedding", "completed", "", "unknown"] {
        assert!(parse_progress(&format!(
            r#"{{"jobId":"job","filename":"a.pdf","status":"{status}"}}"#
        ))
        .is_err());
    }
    assert!(parse_progress(r#"{"filename":"a.pdf","status":"accepted"}"#).is_err());
}

#[test]
fn upload_frames_accept_both_line_endings() {
    assert_eq!(boundary(b"event: parsing\n\n"), Some((14, 2)));
    assert_eq!(boundary(b"event: parsing\r\n\r\n"), Some((14, 4)));
    assert_eq!(boundary(b"event: parsing\r\n"), None);
}
