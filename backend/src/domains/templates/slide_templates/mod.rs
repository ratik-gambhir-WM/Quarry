//! In-process Slide Templates Studio, adapted from the supplied Rust implementation.
pub(super) mod error;
mod facade;
pub mod graph;
mod handler;
mod json_contract;
mod models;
mod normalize;
mod parallel;
pub mod pptx;
mod preview;
mod providers;
pub(crate) mod repository;
pub mod retrieval;
pub(crate) mod route;
mod service;
pub use crate::adapters::slide_templates::RequestCancellation;
pub use error::{FailureKind, SlideResult, SlideTemplateError};
pub use facade::SlideTemplates;
pub use json_contract::{compact_document, normalized_document};
pub use models::*;
pub use preview::*;
pub use providers::*;
pub use repository::{AppRegistration, PostgresTemplateRepository, StoredApp, TemplateRepository};
pub use retrieval::*;
pub use service::{ExportService, ImportService, TEMPLATE_PREVIEW_PAGE_SIZE};
pub(crate) const APP_ID: &str = repository::DEFAULT_APP_ID;

pub(super) fn check_cancelled(cancellation: Option<&RequestCancellation>) -> SlideResult<()> {
    if cancellation.is_some_and(RequestCancellation::is_cancelled) {
        return Err(SlideTemplateError::new(
            FailureKind::Cancelled,
            "request_cancelled",
            "The template import was cancelled.",
        ));
    }
    Ok(())
}
