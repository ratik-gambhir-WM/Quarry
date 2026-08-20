use super::*;

#[test]
fn skipped_jobs_are_terminal_skipped_events() {
    let event = DocumentJobEvent::skipped(
        "job-1".to_string(),
        "memo.pdf".to_string(),
        Some("document-1".to_string()),
    );

    assert_eq!(event.event_name(), "skipped");
    assert!(event.is_terminal());
    assert_eq!(event.document_id.as_deref(), Some("document-1"));
    assert!(event.chunk_count.is_none());
}
