use super::*;

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
