//! In-process slide-template conversion, catalog and classified retrieval.
//!
//! - `facade`: shared engine entry point and bounded execution lifecycle.
//! - `service`: import/export workflows and scoped catalog operations.
//! - `repository`: one PostgreSQL store, split into focused persistence capabilities.
//! - `providers`: classifier/embedder interfaces and OpenAI bindings.
//! - `processing`: classification/embedding orchestration and durable worker.
//! - `retrieval`: metadata contracts, provider input preparation and query ranking.
//! - `pptx`: native PowerPoint package reading, writing and insertion.
pub(super) mod error;
mod facade;
pub(super) mod handler;
mod json_contract;
mod models;
mod normalize;
mod parallel;
pub mod pptx;
mod preview;
mod processing;
mod providers;
pub(crate) mod repository;
pub mod retrieval;
mod service;
pub use crate::adapters::slide_templates::RequestCancellation;
pub use error::{FailureKind, SlideResult, SlideTemplateError};
pub use facade::SlideTemplates;
pub use json_contract::{compact_document, normalized_document};
pub use models::*;
pub use preview::*;
pub use processing::*;
pub use providers::*;
pub use repository::{
    PostgresTemplateRepository, TemplateAssetRepository, TemplateCatalogRepository,
    TemplateProcessingRepository, TemplateRepository, TemplateRetrievalRepository,
};
pub use retrieval::*;
pub use service::{ExportService, ImportService, TEMPLATE_PREVIEW_PAGE_SIZE};

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
