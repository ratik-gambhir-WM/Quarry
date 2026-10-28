use std::sync::Arc;

use crate::{
    adapters::diligence_studio::{
        client::DiligenceStudioClient,
        templates::{
            PptxTemplateImportMode as AdapterImportMode,
            PptxTemplateImportResult as AdapterImportResult, SlideTemplateClientError,
            TemplatePreviewPage as AdapterPage,
        },
    },
    shared::error::{ServiceError, ServiceResult},
};

pub use super::model::*;
#[derive(Clone)]
enum TemplateBackend {
    Unavailable,
    DiligenceStudio(Arc<DiligenceStudioClient>),
    Embedded(Arc<super::slide_templates::SlideTemplates>),
}
#[derive(Clone)]
pub struct TemplateService {
    backend: TemplateBackend,
}
impl TemplateService {
    pub fn new(client: Option<Arc<DiligenceStudioClient>>) -> Self {
        Self {
            backend: client
                .map(TemplateBackend::DiligenceStudio)
                .unwrap_or(TemplateBackend::Unavailable),
        }
    }
    pub fn embedded(studio: Arc<super::slide_templates::SlideTemplates>) -> Self {
        Self {
            backend: TemplateBackend::Embedded(studio),
        }
    }
    pub(crate) fn try_admit(&self) -> ServiceResult<Option<tokio::sync::OwnedSemaphorePermit>> {
        match &self.backend {
            TemplateBackend::Embedded(studio) => {
                studio.try_admit().map(Some).map_err(map_embedded_error)
            }
            TemplateBackend::Unavailable | TemplateBackend::DiligenceStudio(_) => Ok(None),
        }
    }
    fn client(&self) -> Option<&Arc<DiligenceStudioClient>> {
        match &self.backend {
            TemplateBackend::DiligenceStudio(client) => Some(client),
            TemplateBackend::Unavailable | TemplateBackend::Embedded(_) => None,
        }
    }
    pub async fn list_previews(&self, page: usize) -> ServiceResult<TemplatePreviewPage> {
        if let TemplateBackend::Embedded(studio) = &self.backend {
            let result = studio.previews(page).await.map_err(map_embedded_error)?;
            let page = TemplatePreviewPage {
                pagination: TemplatePreviewPagination {
                    page: result.pagination.page,
                    page_size: result.pagination.page_size,
                    total_items: result.pagination.total_items,
                    total_pages: result.pagination.total_pages,
                    has_next_page: result.pagination.has_next_page,
                    has_previous_page: result.pagination.has_previous_page,
                },
                previews: result
                    .previews
                    .into_iter()
                    .map(|p| TemplatePreview {
                        template_id: p.template_id,
                        content_type: p.content_type,
                        data_url: p.data_url,
                        preview_url: p.preview_url,
                        width: p.width,
                        height: p.height,
                    })
                    .collect(),
            };
            validate_embedded_preview_page(&page)?;
            return Ok(page);
        }
        let client = self.client().ok_or_else(|| {
            ServiceError::unavailable("template preview capability is not configured")
        })?;
        client
            .list_template_previews(page)
            .await
            .map(Into::into)
            .map_err(map_slide_template_preview_client_error)
    }

    pub async fn delete(&self, template_id: &str) -> ServiceResult<()> {
        if let TemplateBackend::Embedded(studio) = &self.backend {
            return if studio
                .delete(template_id.into())
                .await
                .map_err(map_embedded_error)?
            {
                Ok(())
            } else {
                Err(ServiceError::not_found("template was not found"))
            };
        }
        let client = self
            .client()
            .ok_or_else(|| ServiceError::unavailable("template capability is not configured"))?;
        client
            .delete_template(template_id)
            .await
            .map_err(map_slide_template_delete_client_error)
    }

    pub async fn get(&self, template_id: &str) -> ServiceResult<serde_json::Value> {
        if let TemplateBackend::Embedded(studio) = &self.backend {
            return studio
                .get(template_id.into())
                .await
                .map_err(map_embedded_error)?
                .ok_or_else(|| ServiceError::not_found("template was not found"));
        }
        let client = self
            .client()
            .ok_or_else(|| ServiceError::unavailable("template capability is not configured"))?;
        client
            .get_template(template_id)
            .await
            .map_err(map_template_document_client_error)
    }

    pub async fn import_pptx_template(
        &self,
        upload: PptxTemplateUpload,
        import_mode: PptxTemplateImportMode,
    ) -> ServiceResult<PptxTemplateImportResult> {
        if let TemplateBackend::Embedded(studio) = &self.backend {
            use super::slide_templates::TemplateKind;
            let (imported_count, warning_count) = match import_mode {
                PptxTemplateImportMode::Single => {
                    let result = studio
                        .import(upload.bytes, TemplateKind::Diagram)
                        .await
                        .map_err(map_embedded_error)?;
                    (1, result.warnings.len())
                }
                PptxTemplateImportMode::Batch => {
                    let result = studio
                        .batch(upload.bytes, TemplateKind::Diagram)
                        .await
                        .map_err(map_embedded_error)?;
                    (result.templates.len(), result.warnings.len())
                }
            };
            if imported_count == 0 || imported_count > 1_000 || warning_count > 10_000 {
                return Err(ServiceError::unavailable(
                    "template import could not be confirmed; refresh templates before retrying",
                ));
            }
            return Ok(PptxTemplateImportResult {
                import_mode,
                imported_count,
                warning_count,
            });
        }
        let client = self
            .client()
            .ok_or_else(|| ServiceError::unavailable("template import is not configured"))?;
        let adapter_mode = match import_mode {
            PptxTemplateImportMode::Single => AdapterImportMode::Single,
            PptxTemplateImportMode::Batch => AdapterImportMode::Batch,
        };
        client
            .import_pptx_template(upload.bytes, adapter_mode)
            .await
            .map(|result| PptxTemplateImportResult::from_adapter(import_mode, result))
            .map_err(|error| map_pptx_import_client_error(error, import_mode))
    }

    pub async fn export_powerpoint(
        &self,
        document: serde_json::Value,
    ) -> ServiceResult<PowerPointExport> {
        if let TemplateBackend::Embedded(studio) = &self.backend {
            let result = studio.export(document).await.map_err(map_embedded_error)?;
            validate_powerpoint_export_size(result.bytes.len())?;
            if result.warnings.len() > 10_000 {
                return Err(ServiceError::unavailable(
                    "PowerPoint export is temporarily unavailable",
                ));
            }
            return Ok(PowerPointExport {
                bytes: result.bytes,
                file_name: result.file_name,
                warning_count: result.warnings.len(),
            });
        }
        let client = self.client().ok_or_else(|| {
            ServiceError::unavailable("PowerPoint export capability is not configured")
        })?;
        let result = client
            .export_powerpoint(&document)
            .await
            .map_err(map_powerpoint_export_client_error)?;
        validate_powerpoint_export_size(result.bytes.len())?;
        Ok(PowerPointExport {
            bytes: result.bytes,
            file_name: result.file_name,
            warning_count: result.warning_count,
        })
    }
}

fn validate_powerpoint_export_size(byte_length: usize) -> ServiceResult<()> {
    if byte_length > MAX_POWERPOINT_EXPORT_BYTES {
        tracing::warn!(byte_length, "PowerPoint export exceeded the response limit");
        return Err(ServiceError::unavailable(
            "PowerPoint export exceeded the 64 MB limit",
        ));
    }
    Ok(())
}

fn map_powerpoint_export_client_error(error: SlideTemplateClientError) -> ServiceError {
    match error {
        SlideTemplateClientError::Status(status) if status.is_client_error() => {
            ServiceError::validation("The presentation could not be exported.")
        }
        error => {
            tracing::warn!(error = %error, "Diligence Studio PowerPoint export request failed");
            ServiceError::unavailable("PowerPoint export is temporarily unavailable")
        }
    }
}

fn map_template_document_client_error(error: SlideTemplateClientError) -> ServiceError {
    if matches!(
        error,
        SlideTemplateClientError::Status(reqwest::StatusCode::NOT_FOUND)
    ) {
        return ServiceError::not_found("template was not found");
    }
    tracing::warn!(error = %error, "Diligence Studio template document request failed");
    ServiceError::unavailable("template is temporarily unavailable")
}

fn map_slide_template_preview_client_error(error: SlideTemplateClientError) -> ServiceError {
    tracing::warn!(error = %error, "Diligence Studio template preview request failed");
    ServiceError::unavailable("template previews are temporarily unavailable")
}

fn map_slide_template_delete_client_error(error: SlideTemplateClientError) -> ServiceError {
    if matches!(
        error,
        SlideTemplateClientError::Status(reqwest::StatusCode::NOT_FOUND)
    ) {
        return ServiceError::not_found("template was not found");
    }
    tracing::warn!(error = %error, "Diligence Studio template delete request failed");
    ServiceError::unavailable("template could not be deleted")
}

fn map_pptx_import_client_error(
    error: SlideTemplateClientError,
    import_mode: PptxTemplateImportMode,
) -> ServiceError {
    match error {
        SlideTemplateClientError::PptxTemplateMustHaveOneSlide => {
            ServiceError::validation(PPTX_MULTI_SLIDE_IMPORT_MESSAGE)
        }
        SlideTemplateClientError::Status(status) if status.is_client_error() => {
            let target = match import_mode {
                PptxTemplateImportMode::Single => "slide template",
                PptxTemplateImportMode::Batch => "deck template",
            };
            ServiceError::validation(format!("The selected {target} could not be imported."))
        }
        SlideTemplateClientError::Request(_) => {
            tracing::warn!("Diligence Studio template import request did not complete");
            ServiceError::unavailable(
                "Template import could not be confirmed. Refresh templates before trying again.",
            )
        }
        error => {
            tracing::warn!(error = %error, "Diligence Studio template import request failed");
            ServiceError::unavailable("template import is temporarily unavailable")
        }
    }
}

impl PptxTemplateImportResult {
    fn from_adapter(import_mode: PptxTemplateImportMode, result: AdapterImportResult) -> Self {
        Self {
            import_mode,
            imported_count: result.imported_count,
            warning_count: result.warning_count,
        }
    }
}

impl From<AdapterPage> for TemplatePreviewPage {
    fn from(page: AdapterPage) -> Self {
        Self {
            pagination: TemplatePreviewPagination {
                page: page.pagination.page,
                page_size: page.pagination.page_size,
                total_items: page.pagination.total_items,
                total_pages: page.pagination.total_pages,
                has_next_page: page.pagination.has_next_page,
                has_previous_page: page.pagination.has_previous_page,
            },
            previews: page
                .previews
                .into_iter()
                .map(|preview| TemplatePreview {
                    template_id: preview.template_id,
                    content_type: preview.content_type,
                    data_url: preview.data_url,
                    preview_url: preview.preview_url,
                    width: preview.width,
                    height: preview.height,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/templates/service_tests.rs"]
mod tests;

fn map_embedded_error(error: super::slide_templates::SlideTemplateError) -> ServiceError {
    use super::slide_templates::error::FailureKind;
    if error.code == "template_must_have_one_slide" {
        return ServiceError::validation(PPTX_MULTI_SLIDE_IMPORT_MESSAGE);
    }
    match error.kind {
        FailureKind::Invalid | FailureKind::TooLarge | FailureKind::Unsupported => {
            ServiceError::validation(error.message)
        }
        FailureKind::NotFound => ServiceError::not_found(error.message),
        _ => {
            tracing::warn!(code=%error.code,"embedded template operation failed");
            ServiceError::unavailable(error.message)
        }
    }
}

fn validate_embedded_preview_page(page: &TemplatePreviewPage) -> ServiceResult<()> {
    let mut pixels = 0u64;
    let mut bytes = 0usize;
    let valid = page.pagination.total_items <= 1_000
        && page.pagination.total_pages <= MAX_TEMPLATE_PREVIEW_PAGES
        && page.previews.len() <= 10
        && page.previews.iter().all(|preview| {
            pixels = pixels.saturating_add(u64::from(preview.width) * u64::from(preview.height));
            bytes = bytes.saturating_add(preview.data_url.len());
            preview.width <= 8_192
                && preview.height <= 8_192
                && preview.data_url.len() <= (8 * 1024 * 1024 * 4 / 3) + 64
        });
    if !valid || pixels > 100_000_000 || bytes > 32 * 1024 * 1024 - 64 * 1024 {
        return Err(ServiceError::unavailable(
            "template previews are temporarily unavailable",
        ));
    }
    Ok(())
}
