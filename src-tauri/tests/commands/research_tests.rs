use super::*;

#[test]
fn activity_log_export_requires_bounded_valid_json() {
    let valid = r#"{"entries":[],"schemaVersion":1}"#;
    assert!(validate_activity_log_payload(valid).is_ok());
    assert_eq!(
        validate_activity_log_payload("not-json"),
        Err("activity log must be valid JSON".to_string())
    );
    assert_eq!(
        validate_activity_log_payload(r#"{"schemaVersion":1}"#),
        Err("activity log must contain an entries array".to_string())
    );

    let oversized = "x".repeat(MAX_ACTIVITY_LOG_BYTES + 1);
    assert_eq!(
        validate_activity_log_payload(&oversized),
        Err("activity log exceeds the 2 MB export limit".to_string())
    );
}

#[test]
fn markdown_summary_export_requires_bounded_content() {
    assert!(validate_summary_content("# Summary").is_ok());
    assert_eq!(
        validate_summary_content("  "),
        Err("summary cannot be empty".to_string())
    );
    assert_eq!(
        validate_summary_content(&"x".repeat(MAX_SUMMARY_EXPORT_BYTES + 1)),
        Err("summary exceeds the 5 MB export limit".to_string())
    );
}
