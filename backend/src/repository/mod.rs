pub mod deal_repository;
pub mod document_repository;
pub mod user_repository;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("{0}")]
    InvalidData(String),
    #[error("{0}")]
    Storage(String),
    #[error("repository blocking worker failed: {0}")]
    BlockingWorker(String),
}

impl RepositoryError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidData(message.into())
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage(message.into())
    }
}
