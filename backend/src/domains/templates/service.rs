use std::sync::Arc;

use crate::shared::error::{ServiceError, ServiceResult};

pub use super::model::*;
#[derive(Clone)]
pub struct TemplateService {
    studio: Option<Arc<super::slide_templates::SlideTemplates>>,
}
impl TemplateService {
    pub fn new(studio: Option<Arc<super::slide_templates::SlideTemplates>>) -> Self {
        Self { studio }
    }
    pub(crate) fn for_user(&self, user_id: i64) -> ServiceResult<Self> {
        match &self.studio {
            Some(studio) => Ok(Self::new(Some(Arc::new(
                studio.for_user(user_id).map_err(map_embedded_error)?,
            )))),
            None => Ok(self.clone()),
        }
    }
    pub(crate) fn try_admit(&self) -> ServiceResult<Option<tokio::sync::OwnedSemaphorePermit>> {
        self.studio
            .as_ref()
            .map(|studio| studio.try_admit().map_err(map_embedded_error))
            .transpose()
    }
    pub async fn list_previews(&self, page: usize) -> ServiceResult<TemplatePreviewPage> {
        let studio = self.studio.as_ref().ok_or_else(|| {
            ServiceError::unavailable("template preview capability is not configured")
        })?;
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
        Ok(page)
    }

    pub async fn delete(&self, template_id: &str) -> ServiceResult<()> {
        let studio = self
            .studio
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("template capability is not configured"))?;
        if studio
            .delete(template_id.into())
            .await
            .map_err(map_embedded_error)?
        {
            Ok(())
        } else {
            Err(ServiceError::not_found("template was not found"))
        }
    }

    pub async fn get(&self, template_id: &str) -> ServiceResult<serde_json::Value> {
        let studio = self
            .studio
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("template capability is not configured"))?;
        studio
            .get(template_id.into())
            .await
            .map_err(map_embedded_error)?
            .ok_or_else(|| ServiceError::not_found("template was not found"))
    }

    pub async fn import_pptx_template(
        &self,
        upload: PptxTemplateUpload,
        import_mode: PptxTemplateImportMode,
    ) -> ServiceResult<PptxTemplateImportResult> {
        let studio = self
            .studio
            .as_ref()
            .ok_or_else(|| ServiceError::unavailable("template import is not configured"))?;
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
        Ok(PptxTemplateImportResult {
            import_mode,
            imported_count,
            warning_count,
        })
    }

    pub async fn export_powerpoint(
        &self,
        document: serde_json::Value,
    ) -> ServiceResult<PowerPointExport> {
        let studio = self.studio.as_ref().ok_or_else(|| {
            ServiceError::unavailable("PowerPoint export capability is not configured")
        })?;
        let result = studio.export(document).await.map_err(map_embedded_error)?;
        validate_powerpoint_export_size(result.bytes.len())?;
        if result.warnings.len() > 10_000 {
            return Err(ServiceError::unavailable(
                "PowerPoint export is temporarily unavailable",
            ));
        }
        Ok(PowerPointExport {
            bytes: result.bytes,
            file_name: result.file_name,
            warning_count: result.warnings.len(),
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
