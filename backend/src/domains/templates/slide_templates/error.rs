//! Typed conversion failures; HTTP mapping belongs to the handler.
use std::borrow::Cow;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    Invalid,
    Conflict,
    TooLarge,
    Unsupported,
    NotFound,
    Unavailable,
    Timeout,
    Upstream,
    Cancelled,
    Internal,
}

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct SlideTemplateError {
    pub kind: FailureKind,
    pub code: Cow<'static, str>,
    pub message: &'static str,
    #[source]
    source: Option<anyhow::Error>,
}
impl SlideTemplateError {
    pub fn new(
        kind: FailureKind,
        code: impl Into<Cow<'static, str>>,
        message: &'static str,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message,
            source: None,
        }
    }
    pub fn repository(error: crate::shared::error::RepositoryError) -> Self {
        match error {
            crate::shared::error::RepositoryError::Conflict(_) => Self::new(
                FailureKind::Conflict,
                "revision_conflict",
                "This template changed. Refresh it before editing.",
            ),
            other => Self::internal(other),
        }
    }
    pub fn internal(source: impl Into<anyhow::Error>) -> Self {
        Self::new(
            FailureKind::Internal,
            "internal_error",
            "The request could not be completed.",
        )
        .with_source(source)
    }
    pub fn with_source(mut self, source: impl Into<anyhow::Error>) -> Self {
        self.source = Some(source.into());
        self
    }
}
pub type SlideResult<T> = Result<T, SlideTemplateError>;
