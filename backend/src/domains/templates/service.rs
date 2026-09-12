use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    adapters::diligence_studio::{
        client::DiligenceStudioClient,
        templates::{
            PptxTemplateImportMode as AdapterImportMode,
            PptxTemplateImportResult as AdapterImportResult, SlideTemplateClientError,
            TemplatePreviewPage as AdapterPage, MAX_TEMPLATE_ID_BYTES, MAX_TEMPLATE_PREVIEW_PAGES,
        },
    },
    shared::error::{ServiceError, ServiceResult},
};

pub const PPTX_MULTI_SLIDE_IMPORT_MESSAGE: &str =
    "This PowerPoint contains multiple slides. Use Import Deck Template instead.";
pub const MAX_PPTX_TEMPLATE_IMPORT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PptxTemplateImportMode {
    Single,
    Batch,
}

#[derive(Debug)]
pub struct PptxTemplateUpload {
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxTemplateImportResult {
    pub import_mode: PptxTemplateImportMode,
    pub imported_count: usize,
    pub warning_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewPage {
    pub pagination: TemplatePreviewPagination,
    pub previews: Vec<TemplatePreview>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewPagination {
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreview {
    pub template_id: String,
    pub content_type: String,
    pub data_url: String,
    pub preview_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone)]
pub struct TemplateService {
    client: Option<Arc<DiligenceStudioClient>>,
}

impl TemplateService {
    pub fn new(client: Option<Arc<DiligenceStudioClient>>) -> Self {
        Self { client }
    }

    pub async fn list_previews(&self, page: usize) -> ServiceResult<TemplatePreviewPage> {
        if page == 0 || page > MAX_TEMPLATE_PREVIEW_PAGES {
            return Err(ServiceError::validation(
                "page is outside the supported range",
            ));
        }
        let client = self.client.as_ref().ok_or_else(|| {
            ServiceError::unavailable("template preview capability is not configured")
        })?;
        client
            .list_template_previews(page)
            .await
            .map(Into::into)
            .map_err(map_slide_template_preview_client_error)
    }

    pub async fn delete(&self, template_id: &str) -> ServiceResult<()> {
        if template_id.trim().is_empty()
            || template_id.len() > MAX_TEMPLATE_ID_BYTES
            || template_id.chars().any(char::is_control)
        {
            return Err(ServiceError::validation("template ID is invalid"));
        }
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("template capability is not configured"))?;
        client
            .delete_template(template_id)
            .await
            .map_err(map_slide_template_delete_client_error)
    }

    pub async fn import_pptx_template(
        &self,
        upload: PptxTemplateUpload,
        import_mode: PptxTemplateImportMode,
    ) -> ServiceResult<PptxTemplateImportResult> {
        if upload.bytes.is_empty() || upload.bytes.len() > MAX_PPTX_TEMPLATE_IMPORT_BYTES {
            return Err(ServiceError::validation(
                "PPTX template bytes are empty or exceed the 25 MB limit",
            ));
        }
        let client = self
            .client
            .as_ref()
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
