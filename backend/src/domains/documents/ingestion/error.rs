//! Processing failures retain canonical commit state independently of diagnostics.

#[derive(Debug, thiserror::Error)]
pub(crate) enum IngestionError {
    #[error("{0}")]
    BeforeCommit(String),
    #[error("{0}")]
    AfterCommit(String),
}

impl IngestionError {
    pub(crate) fn sqlite_committed(&self) -> bool {
        matches!(self, Self::AfterCommit(_))
    }
}

impl From<String> for IngestionError {
    fn from(message: String) -> Self {
        Self::BeforeCommit(message)
    }
}

impl From<&str> for IngestionError {
    fn from(message: &str) -> Self {
        Self::BeforeCommit(message.to_owned())
    }
}
