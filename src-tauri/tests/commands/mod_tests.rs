use super::*;

#[test]
fn command_context_preserves_success_values() {
    assert_eq!(Ok::<_, String>(42).command_context("example"), Ok(42));
}

#[test]
fn command_context_returns_a_safe_structured_error() {
    let error = Err::<(), _>("sqlite error at /Users/example/private.sqlite")
        .command_context("example")
        .expect_err("error should be mapped");

    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(error.message, "Example could not be completed.");
    assert!(error.operation_id.starts_with("example-"));
    let serialized = serde_json::to_string(&error).expect("error should serialize");
    assert!(!serialized.contains("sqlite"));
    assert!(!serialized.contains("/Users/example"));
}

#[test]
fn validation_context_keeps_safe_actionable_copy() {
    let error = Err::<(), _>("dealId must be a positive integer")
        .validation_context("get_deal")
        .expect_err("validation should fail");

    assert_eq!(error.code, ErrorCode::Validation);
    assert_eq!(error.message, "dealId must be a positive integer");
    assert!(!error.retryable);
}
