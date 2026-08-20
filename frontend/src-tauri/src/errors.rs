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
        let error = Self::new(
            ErrorCode::Internal,
            "The file could not be saved.".to_string(),
            false,
        );
        eprintln!("[{}] save_text_file failed: {}", error.operation_id, source);
        error
    }

    fn new(code: ErrorCode, message: String, retryable: bool) -> Self {
        let sequence = NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            code,
            message,
            operation_id: format!("save-text-file-{sequence}"),
            retryable,
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
