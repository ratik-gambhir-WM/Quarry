//! Map domain canvas documents to the native preview contract.
use super::{CanvasDocument, RequestCancellation};
use crate::adapters::slide_templates::preview as native;
pub use crate::adapters::slide_templates::preview::GeneratedTemplatePreview;
use std::sync::Arc;
pub trait TemplatePreviewGenerator: Send + Sync {
    fn supports_independent_slides(&self) -> bool {
        true
    }
    fn generate(&self, source: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>>;
    fn generate_document(
        &self,
        source: &[u8],
        document: &CanvasDocument,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        self.generate_document_cancellable(source, document, None)
    }
    fn generate_document_cancellable(
        &self,
        source: &[u8],
        document: &CanvasDocument,
        cancellation: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        let _ = document;
        if cancellation.is_some_and(RequestCancellation::is_cancelled) {
            return Ok(None);
        }
        self.generate(source)
    }
}
pub struct NativePreview(pub Arc<dyn native::TemplatePreviewGenerator>);
impl TemplatePreviewGenerator for NativePreview {
    fn supports_independent_slides(&self) -> bool {
        self.0.supports_independent_slides()
    }
    fn generate(&self, source: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        if self.0.requires_document() {
            let document = super::pptx::read(source)?.template_json;
            self.0
                .generate_document(source, &serde_json::to_value(document)?)
        } else {
            self.0.generate(source)
        }
    }
    fn generate_document_cancellable(
        &self,
        source: &[u8],
        document: &CanvasDocument,
        cancellation: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        self.0
            .generate_document_cancellable(source, &serde_json::to_value(document)?, cancellation)
    }
}
pub struct DisabledTemplatePreviewGenerator;
impl TemplatePreviewGenerator for DisabledTemplatePreviewGenerator {
    fn generate(&self, _: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        Ok(None)
    }
    fn generate_document_cancellable(
        &self,
        _: &[u8],
        _: &CanvasDocument,
        _: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        Ok(None)
    }
}
