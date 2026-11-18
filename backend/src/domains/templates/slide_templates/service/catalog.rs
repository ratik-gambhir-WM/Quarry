//! Catalog reads, previews, deletion, inspection and manual retry.
use super::super::{
    retrieval::*, FailureKind, SlideResult, SlideTemplateError, StoredTemplate,
    StoredTemplatePreview, TemplateAsset, TemplateKind, TemplateListItem, TemplateListResponse,
    TemplatePreviewListItem, TemplatePreviewListResponse, TemplatePreviewPageMetadata,
    TemplateResponse, TemplateSource,
};
use super::assets::{externalize_assets, hydrate_canvas_assets};
use super::{
    blocking, document_too_large, repair_canvas_dimensions, ImportService,
    TEMPLATE_PREVIEW_PAGE_SIZE,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;

impl ImportService {
    /// Retrieves a stored template and lazily migrates legacy images and line shapes.
    pub async fn find(&self, template_id: &str) -> SlideResult<Option<TemplateResponse>> {
        let Some(stored) = self
            .repository
            .find_by_id_with_assets(template_id)
            .await
            .map_err(SlideTemplateError::internal)?
        else {
            return Ok(None);
        };
        let id = template_id.to_owned();
        let (mut stored, migrated_assets, update) = blocking(move || {
            let mut stored = stored;
            let (mut document, migrated_assets) = externalize_assets(&id, stored.template_json)?;
            let repaired = repair_canvas_dimensions(&mut document);
            let update = (repaired || !migrated_assets.is_empty()).then(|| StoredTemplate {
                template_id: id,
                template_json: document.clone(),
                revision: stored.revision,
            });
            stored.template_json = document;
            Ok((stored, migrated_assets, update))
        })
        .await?;
        if let Some(update) = update {
            self.repository
                .update(&update, &migrated_assets)
                .await
                .map_err(SlideTemplateError::repository)?;
            stored.revision = crate::shared::revision::next(stored.revision)
                .map_err(SlideTemplateError::repository)?;
        }
        blocking(move || {
            stored.assets.extend(migrated_assets);
            Ok(Some(TemplateResponse {
                revision: stored.revision,
                template_id: stored.template_id,
                template_json: hydrate_canvas_assets(stored.template_json, &stored.assets)?,
            }))
        })
        .await
    }

    pub async fn find_json(&self, template_id: &str) -> SlideResult<Option<Value>> {
        let Some(template) = self.find(template_id).await? else {
            return Ok(None);
        };
        let builtin = self
            .repository
            .find_metadata(template_id)
            .await
            .map_err(SlideTemplateError::internal)?
            .is_some_and(|metadata| metadata.source == TemplateSource::Builtin);
        blocking(move || {
            let document = if builtin {
                super::super::normalized_document(&template.template_json)
            } else {
                super::super::compact_document(&template.template_json)
            };
            Ok(Some(
                serde_json::json!({ "revision": template.revision, "document": document }),
            ))
        })
        .await
    }

    /// Lists lightweight metadata for every stored template.
    pub async fn list(&self, kind: Option<TemplateKind>) -> SlideResult<TemplateListResponse> {
        let templates = self
            .repository
            .list(kind)
            .await
            .map_err(SlideTemplateError::internal)?
            .into_iter()
            .map(|entry| {
                let presentation = entry.template.template_json.presentation;
                TemplateListItem {
                    revision: entry.template.revision,
                    template_id: entry.template.template_id.clone(),
                    title: presentation.title,
                    description: entry.metadata.description,
                    kind: entry.metadata.kind,
                    slide_count: presentation.slides.len(),
                    element_count: presentation
                        .slides
                        .iter()
                        .map(|slide| slide.elements.len())
                        .sum(),
                    preview_url: entry.has_preview.then(|| {
                        format!(
                            "/api/v1/templates/slide-templates/catalog/{}/preview",
                            entry.template.template_id
                        )
                    }),
                }
            })
            .collect();
        Ok(TemplateListResponse { templates })
    }

    /// Lists preview PNGs in fixed pages without hydrating template documents.
    pub async fn list_previews(&self, page: usize) -> SlideResult<TemplatePreviewListResponse> {
        let offset = page
            .checked_sub(1)
            .and_then(|page_index| page_index.checked_mul(TEMPLATE_PREVIEW_PAGE_SIZE))
            .ok_or_else(invalid_preview_page)?;
        let stored = self
            .repository
            .list_previews(TEMPLATE_PREVIEW_PAGE_SIZE, offset)
            .await
            .map_err(SlideTemplateError::internal)?;
        blocking(move || {
            let total_pages = stored.total.div_ceil(TEMPLATE_PREVIEW_PAGE_SIZE);
            let encoded_size = stored
                .previews
                .iter()
                .map(|p| p.bytes.len().saturating_mul(4).div_ceil(3) + 128)
                .fold(0usize, usize::saturating_add);
            if encoded_size > super::super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES {
                return Err(document_too_large());
            }
            let previews = stored
                .previews
                .into_iter()
                .map(|preview| TemplatePreviewListItem {
                    revision: preview.revision,
                    preview_url: format!(
                        "/api/v1/templates/slide-templates/catalog/{}/preview",
                        preview.template_id
                    ),
                    data_url: format!(
                        "data:{};base64,{}",
                        preview.content_type,
                        STANDARD.encode(&preview.bytes)
                    ),
                    template_id: preview.template_id,
                    content_type: preview.content_type,
                    width: preview.width,
                    height: preview.height,
                })
                .collect::<Vec<_>>();
            Ok(TemplatePreviewListResponse {
                pagination: TemplatePreviewPageMetadata {
                    page,
                    page_size: TEMPLATE_PREVIEW_PAGE_SIZE,
                    total_items: stored.total,
                    total_pages,
                    has_previous_page: page > 1 && stored.total > 0,
                    has_next_page: offset.saturating_add(previews.len()) < stored.total,
                },
                previews,
            })
        })
        .await
    }

    /// Deletes a stored template and its associated assets.
    pub async fn delete(&self, template_id: &str, expected_revision: i64) -> SlideResult<bool> {
        self.repository
            .delete(template_id, expected_revision)
            .await
            .map_err(SlideTemplateError::repository)
    }

    /// Retrieves a template-owned binary image asset.
    pub async fn find_asset(
        &self,
        template_id: &str,
        asset_id: &str,
    ) -> SlideResult<Option<TemplateAsset>> {
        self.repository
            .find_asset(template_id, asset_id)
            .await
            .map_err(SlideTemplateError::internal)
    }

    /// Retrieves the stored first-slide PNG preview for a template.
    pub async fn find_preview(
        &self,
        template_id: &str,
    ) -> SlideResult<Option<StoredTemplatePreview>> {
        self.repository
            .find_preview(template_id)
            .await
            .map_err(SlideTemplateError::internal)
    }

    /// Checks whether a template exists without hydrating its assets.
    pub async fn has_template(&self, template_id: &str) -> SlideResult<bool> {
        self.repository
            .find_by_id(template_id)
            .await
            .map(|template| template.is_some())
            .map_err(SlideTemplateError::internal)
    }

    /// Requeue only a failed stage, preserving the saved slide and completed artifacts.
    pub async fn retry_failed_processing(
        &self,
        template_id: &str,
    ) -> SlideResult<RetrySlideProcessingResponse> {
        let outcome = self
            .repository
            .retry_failed_slide_processing(template_id)
            .await
            .map_err(SlideTemplateError::internal)?;
        match outcome {
            RetrySlideProcessingOutcome::Queued(stage) => Ok(RetrySlideProcessingResponse {
                template_id: template_id.to_owned(),
                stage,
            }),
            RetrySlideProcessingOutcome::NotFound => Err(SlideTemplateError::new(
                FailureKind::NotFound,
                "template_not_found",
                "Template was not found.",
            )),
            RetrySlideProcessingOutcome::NotFailed => Err(SlideTemplateError::new(
                FailureKind::Conflict,
                "slide_processing_not_failed",
                "Only failed slide processing can be retried. Refresh its status.",
            )),
        }
    }

    /// Reads the opt-in v2 retrieval fields without hydrating the template document.
    ///
    /// The caller must have already selected an user-scoped service, so checking the template
    /// first prevents classification data from being exposed across user boundaries.
    pub async fn find_v2(
        &self,
        template_id: &str,
        include_metadata: bool,
        include_embeddings: bool,
    ) -> SlideResult<Option<Value>> {
        if !self.has_template(template_id).await? {
            return Ok(None);
        }
        let mut result = serde_json::json!({"templateId": template_id});
        let classification = self
            .repository
            .find_slide_classification(template_id)
            .await
            .map_err(SlideTemplateError::internal)?;
        result["retrievalStatus"] = serde_json::json!(classification
            .as_ref()
            .map_or("not_ready", |record| record.status.as_str()));
        result["classification"] = serde_json::json!({
            "status": classification.as_ref().map_or("not_ready", |r| r.status.as_str()),
            "attemptCount": classification.as_ref().map_or(0, |r| r.attempt_count),
            "nextAttemptAt": classification.as_ref().and_then(|r| r.next_attempt_at.as_ref()),
            "lastErrorCode": classification.as_ref().and_then(|r| r.last_error_code.as_ref()),
        });
        result["embedding"] = serde_json::json!({
            "status": classification.as_ref().map_or("not_ready", |r| r.embedding_status.as_str()),
            "attemptCount": classification.as_ref().map_or(0, |r| r.embedding_attempt_count),
            "nextAttemptAt": classification.as_ref().and_then(|r| r.embedding_next_attempt_at.as_ref()),
            "lastErrorCode": classification.as_ref().and_then(|r| r.embedding_last_error_code.as_ref()),
        });
        if include_metadata {
            result["metadata"] = classification
                .as_ref()
                .and_then(|record| record.metadata.as_ref())
                .map(serde_json::to_value)
                .transpose()
                .map_err(SlideTemplateError::internal)?
                .unwrap_or(Value::Null);
        }
        if include_embeddings {
            result["embeddings"] = serde_json::json!({
                "subjectVector": classification.as_ref().and_then(|record| record.subject_vector.as_ref()),
                "capabilityVector": classification.as_ref().and_then(|record| record.capability_vector.as_ref()),
                "dimensions": classification.as_ref().and_then(|record| record.embedding_dimensions),
                "model": classification.as_ref().and_then(|record| record.embedding_model.as_ref()),
                "status": classification.as_ref().map_or("not_ready", |record| record.embedding_status.as_str()),
            });
        }
        Ok(Some(result))
    }
}
fn invalid_preview_page() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::Invalid,
        "invalid_preview_page",
        "Preview page must be a positive integer.",
    )
}
