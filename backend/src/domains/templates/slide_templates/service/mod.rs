//! Shared service handles, scope selection and common workflow helpers.

mod assets;
mod catalog;
mod export;
mod import;
pub use export::ExportService;
pub(super) use import::classified_batch_json;

use super::{
    CanvasDocument, DisabledTemplatePreviewGenerator, Element, FailureKind, SlideResult,
    SlideTemplateError, TemplatePreviewGenerator, TemplateRepository,
};
use std::sync::Arc;

pub const TEMPLATE_PREVIEW_PAGE_SIZE: usize = 10;

/// Converts uploaded PowerPoint packages and persists templates plus image assets.
#[derive(Clone)]
pub struct ImportService {
    repository: Arc<dyn TemplateRepository>,
    preview_generator: Arc<dyn TemplatePreviewGenerator>,
}

impl ImportService {
    pub(super) fn exporter(&self) -> ExportService {
        ExportService::new(self.repository.clone())
    }

    pub(super) fn for_user(&self, user_id: i64) -> SlideResult<Self> {
        Ok(Self {
            repository: self
                .repository
                .for_user(user_id)
                .map_err(SlideTemplateError::internal)?,
            ..self.clone()
        })
    }

    #[must_use]
    pub fn new(repository: Arc<dyn TemplateRepository>) -> Self {
        Self {
            repository,
            preview_generator: Arc::new(DisabledTemplatePreviewGenerator),
        }
    }

    #[must_use]
    pub fn with_preview_generator(
        repository: Arc<dyn TemplateRepository>,
        preview_generator: Arc<dyn TemplatePreviewGenerator>,
    ) -> Self {
        Self {
            repository,
            preview_generator,
        }
    }
}

fn repair_canvas_dimensions(document: &mut CanvasDocument) -> bool {
    let mut repaired = false;
    for slide in &mut document.presentation.slides {
        repaired |= repair_positive_dimension(&mut slide.width);
        repaired |= repair_positive_dimension(&mut slide.height);
        for element in &mut slide.elements {
            match element {
                Element::Text(element) => {
                    repaired |= repair_positive_dimension(&mut element.w);
                    repaired |= repair_positive_dimension(&mut element.h);
                }
                Element::Shape(element) => {
                    repaired |= repair_positive_dimension(&mut element.w);
                    repaired |= repair_positive_dimension(&mut element.h);
                }
                Element::Image(element) => {
                    repaired |= repair_positive_dimension(&mut element.w);
                    repaired |= repair_positive_dimension(&mut element.h);
                }
                Element::Line(_) => {}
            }
        }
    }
    repaired
}

fn repair_positive_dimension(value: &mut f64) -> bool {
    if *value > 0.0 {
        return false;
    }
    *value = 1.0;
    true
}

fn document_too_large() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::TooLarge,
        "template_too_large",
        "The template exceeds the processing limit.",
    )
}

fn map_repository_failure(error: crate::shared::error::RepositoryError) -> SlideTemplateError {
    let error = anyhow::Error::from(error);
    if error
        .chain()
        .any(|cause| cause.is::<super::repository::PublicationCancelled>())
    {
        SlideTemplateError::new(
            FailureKind::Cancelled,
            "request_cancelled",
            "Template processing was cancelled.",
        )
    } else {
        SlideTemplateError::internal(error)
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> SlideResult<T> + Send + 'static,
) -> SlideResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(SlideTemplateError::internal)?
}
