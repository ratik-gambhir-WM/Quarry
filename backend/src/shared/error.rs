use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("save queue capacity exhausted")]
    Capacity,
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    InvalidData(String),
    #[error("{0}")]
    Storage(String),
    #[error("repository storage operation failed")]
    StorageCause(#[source] anyhow::Error),
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ServiceError {
    #[error("invalid credentials or expired session")]
    Unauthorized,
    #[error("too many authentication attempts; retry later")]
    RateLimited,
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Internal(String),
}

impl ServiceError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

impl From<RepositoryError> for ServiceError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::Capacity => Self::Unavailable("save queue capacity exhausted".into()),
            RepositoryError::NotFound(message) => Self::NotFound(message),
            RepositoryError::Conflict(message) => Self::Conflict(message),
            RepositoryError::InvalidData(message) => Self::Validation(message),
            error => Self::Internal(error.to_string()),
        }
    }
}

pub type ServiceResult<T> = Result<T, ServiceError>;

impl From<anyhow::Error> for RepositoryError {
    fn from(source: anyhow::Error) -> Self {
        Self::StorageCause(source)
    }
}

impl From<crate::adapters::sqlite::client::SqliteClientError> for RepositoryError {
    fn from(source: crate::adapters::sqlite::client::SqliteClientError) -> Self {
        Self::StorageCause(source.into())
    }
}
