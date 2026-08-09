use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Conflict,
    Internal,
    NotFound,
    Permission,
    ServiceUnavailable,
    Validation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub operation_id: String,
    pub retryable: bool,
}

impl AppError {
    pub fn validation(operation: &str, message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Validation, operation, message.into(), false)
    }

    pub fn not_found(operation: &str, message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, operation, message.into(), false)
    }

    pub fn from_source(operation: &str, source: impl std::fmt::Display) -> Self {
        let source = source.to_string();
        let normalized = source.to_ascii_lowercase();

        let (code, message, retryable) = if normalized.contains("outside the")
            || normalized.contains("path traversal")
            || normalized.contains("permission denied")
        {
            (
                ErrorCode::Permission,
                "The requested file operation is not permitted.".to_string(),
                false,
            )
        } else if normalized.contains("unavailable")
            || normalized.contains("does not exist")
            || normalized.contains("not found")
            || normalized.contains("no local data-room root")
        {
            (
                ErrorCode::NotFound,
                "The requested local resource is unavailable. Reconnect it and try again."
                    .to_string(),
                true,
            )
        } else if normalized.contains("openai")
            || normalized.contains("helix")
            || normalized.contains("timed out")
            || normalized.contains("connection")
        {
            (
                ErrorCode::ServiceUnavailable,
                "A required service is unavailable. Try again shortly.".to_string(),
                true,
            )
        } else {
            (
                ErrorCode::Internal,
                format!("{} could not be completed.", humanize_operation(operation)),
                false,
            )
        };

        let error = Self::new(code, operation, message, retryable);
        eprintln!(
            "[{}] {} failed with {:?}",
            error.operation_id, operation, error.code
        );
        error
    }

    fn new(code: ErrorCode, operation: &str, message: String, retryable: bool) -> Self {
        let sequence = NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            code,
            message,
            operation_id: format!("{operation}-{sequence}"),
            retryable,
        }
    }
}

fn humanize_operation(operation: &str) -> String {
    let mut words = operation.replace('_', " ");
    if let Some(first) = words.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    words
}

pub type AppResult<T> = Result<T, AppError>;
