use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Internal,
    Permission,
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
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Validation, message.into(), false)
    }

    pub fn permission(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Permission, message.into(), false)
    }

    pub fn internal(source: impl std::fmt::Display) -> Self {
        Self::internal_operation("save-text-file", "The file could not be saved.", source)
    }

    pub fn internal_operation(
        operation: &'static str,
        public_message: &'static str,
        source: impl std::fmt::Display,
    ) -> Self {
        let error = Self::new(ErrorCode::Internal, public_message.to_string(), false);
        eprintln!("[{}] {operation} failed: {source}", error.operation_id);
        error
    }

    fn new(code: ErrorCode, message: String, retryable: bool) -> Self {
        let sequence = NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            code,
            message,
            operation_id: format!("operation-{sequence}"),
            retryable,
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
