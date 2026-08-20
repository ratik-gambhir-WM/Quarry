use serde_json::Value;
use thiserror::Error;

/// Error type for all SharePoint client failures.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct SharePointClientError {
    message: String,
    pub status_code: u16,
    pub details: Option<Value>,
}

impl SharePointClientError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status_code: 513,
            details: None,
        }
    }

    pub fn with_status(message: impl Into<String>, status_code: u16) -> Self {
        Self {
            message: message.into(),
            status_code,
            details: None,
        }
    }

    pub fn with_details(
        message: impl Into<String>,
        status_code: u16,
        details: impl Into<Value>,
    ) -> Self {
        Self {
            message: message.into(),
            status_code,
            details: Some(details.into()),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<reqwest::Error> for SharePointClientError {
    fn from(error: reqwest::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<serde_json::Error> for SharePointClientError {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}
