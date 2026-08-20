use super::*;

#[test]
fn recognizes_only_concurrent_write_conflicts_as_retryable() {
    let conflict = HelixError::RemoteError {
        details: r#"{"error":"request conflicted with a concurrent write; please retry"}"#
            .to_string(),
    };
    let other_error = HelixError::RemoteError {
        details: r#"{"error":"unique constraint violation"}"#.to_string(),
    };

    assert!(is_concurrent_write_conflict(&conflict));
    assert!(!is_concurrent_write_conflict(&other_error));
}

#[test]
fn write_retry_delay_uses_bounded_exponential_backoff() {
    assert_eq!(write_retry_delay(1), Duration::from_millis(25));
    assert_eq!(write_retry_delay(2), Duration::from_millis(50));
    assert_eq!(write_retry_delay(4), Duration::from_millis(200));
}
