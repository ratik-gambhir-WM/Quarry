use quarry_lib::document_jobs::{DocumentJobEvent, DocumentJobManager};

#[tokio::test]
async fn document_job_status_survives_until_pruned() {
    let manager = DocumentJobManager::with_capacity(2);
    let processing = DocumentJobEvent::processing("job-1".into(), "memo.pdf".into());
    manager.insert(processing.clone()).await;
    assert_eq!(manager.get("job-1").await, Some(processing));

    let completed = DocumentJobEvent::completed(
        "job-1".into(),
        "memo.pdf".into(),
        Some("document-1".into()),
        4,
    );
    manager.update(completed.clone()).await;
    assert_eq!(manager.get("job-1").await, Some(completed));
}

#[tokio::test]
async fn retention_never_prunes_in_flight_jobs() {
    let manager = DocumentJobManager::with_capacity(1);
    manager
        .insert(DocumentJobEvent::processing(
            "job-1".into(),
            "one.pdf".into(),
        ))
        .await;
    manager
        .insert(DocumentJobEvent::processing(
            "job-2".into(),
            "two.pdf".into(),
        ))
        .await;
    assert!(manager.get("job-1").await.is_some());
    assert!(manager.get("job-2").await.is_some());

    manager
        .update(DocumentJobEvent::failed(
            "job-1".into(),
            "one.pdf".into(),
            "Safe failure".into(),
        ))
        .await;
    assert!(manager.get("job-1").await.is_none());
    assert!(manager.get("job-2").await.is_some());
}

#[test]
fn safe_job_failures_never_serialize_paths_or_service_details() {
    let event = DocumentJobEvent::failed(
        "job-1".into(),
        "memo.pdf".into(),
        "Document processing failed. Try again.".into(),
    );
    let serialized = serde_json::to_string(&event).unwrap();

    assert!(!serialized.contains("/Users/"));
    assert!(!serialized.contains("OPENAI_API_KEY"));
    assert!(event.is_terminal());
}
