//! Application services for import, retrieval, asset storage, and export.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

mod assets;

use super::error::FailureKind;
use super::retrieval::*;
use super::{check_cancelled, RequestCancellation};
use assets::{
    externalize_assets, hydrate_canvas_assets, hydrate_template_assets, validate_image_sources,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::Value;
use uuid::Uuid;

use super::{
    normalize, pptx, BatchImportResponse, BatchImportTemplate, CanvasDocument, Conversion,
    DisabledTemplatePreviewGenerator, Element, ExportedPowerPoint, ImportResponse,
    InsertPowerPointInput, Presentation, SlideResult, SlideTemplateError, StoredTemplate,
    StoredTemplateMetadata, StoredTemplatePreview, TemplateAsset, TemplateInsert, TemplateKind,
    TemplateListItem, TemplateListResponse, TemplatePreviewGenerator, TemplatePreviewListItem,
    TemplatePreviewListResponse, TemplatePreviewPageMetadata, TemplatePreviewStatus,
    TemplateRepository, TemplateResponse, TemplateSource,
};

const MAX_JSON_DEPTH: usize = 100;
const MAX_JSON_NODES: usize = 100_000;
pub const TEMPLATE_PREVIEW_PAGE_SIZE: usize = 10;

/// Converts presentation JSON into downloadable `.pptx` bytes.
#[derive(Clone)]
pub struct ExportService {
    repository: Arc<dyn TemplateRepository>,
}

impl ExportService {
    pub(super) fn for_user(&self, user_id: i64) -> SlideResult<Self> {
        Ok(Self {
            repository: self
                .repository
                .for_user(user_id)
                .map_err(SlideTemplateError::internal)?,
        })
    }

    #[must_use]
    pub fn new(repository: Arc<dyn TemplateRepository>) -> Self {
        Self { repository }
    }

    pub async fn for_app(&self, app_id: &str) -> SlideResult<Self> {
        Ok(Self {
            repository: self
                .repository
                .for_app(app_id)
                .await
                .map_err(SlideTemplateError::internal)?
                .unwrap_or_else(|| Arc::clone(&self.repository)),
        })
    }

    /// Normalizes, validates, hydrates, and renders a presentation.
    ///
    /// # Errors
    ///
    /// Returns a stable API error for complex/invalid JSON, unsafe images, missing assets,
    /// oversized embedded images, or rendering failures.
    pub async fn export(&self, input: Value) -> SlideResult<ExportedPowerPoint> {
        let (mut presentation, warnings) = blocking(move || {
            assert_bounded_json(&input)?;
            let (mut presentation, warnings) = normalize::normalize(&input)?;
            // The reference's rendering normalizer drops empty image sources after canvas
            // normalization, without adding to the export's canvas-normalization warnings.
            for slide in &mut presentation.presentation.slides {
                slide.elements.retain(
                    |element| !matches!(element, Element::Image(image) if image.src.is_empty()),
                );
            }
            Ok((presentation, warnings))
        })
        .await?;
        hydrate_template_assets(&mut presentation, self.repository.as_ref()).await?;
        let (bytes, file_name) = blocking(move || {
            validate_image_sources(&presentation)?;
            let file_name = suggested_file_name(&presentation.presentation.title);
            let bytes = pptx::write(&presentation).map_err(SlideTemplateError::internal)?;
            Ok((bytes, file_name))
        })
        .await?;
        Ok(ExportedPowerPoint {
            bytes,
            file_name,
            warnings,
        })
    }

    /// Renders the supplied presentation and inserts it into an existing `.pptx` package.
    ///
    /// # Errors
    ///
    /// Returns a stable API error when the target deck or insert position is invalid, or when
    /// rendering/copying fails.
    pub async fn insert(&self, input: InsertPowerPointInput) -> SlideResult<ExportedPowerPoint> {
        if input.insert_after_slide > 9_007_199_254_740_991 {
            return Err(SlideTemplateError::new(
                FailureKind::Invalid,
                "invalid_insert_position",
                "insertAfterSlide must be a non-negative whole number.",
            ));
        }
        let target = blocking(move || {
            pptx::validate_package(&input.target_bytes)?;
            Ok(input.target_bytes)
        })
        .await?;
        let generated = self.export(input.presentation).await?;
        let generated_bytes = generated.bytes;
        let bytes = blocking(move || {
            pptx::insert(&generated_bytes, &target, input.insert_after_slide).map_err(|error| {
                SlideTemplateError::new(
                    FailureKind::Invalid,
                    "invalid_target_powerpoint",
                    "The target file is not a supported PowerPoint presentation.",
                )
                .with_source(error)
            })
        })
        .await?;
        Ok(ExportedPowerPoint {
            bytes,
            file_name: inserted_file_name(&input.target_file_name),
            warnings: generated.warnings,
        })
    }
}

/// Converts uploaded PowerPoint packages and persists templates plus image assets.
#[derive(Clone)]
pub struct ImportService {
    app_id: String,
    repository: Arc<dyn TemplateRepository>,
    preview_generator: Arc<dyn TemplatePreviewGenerator>,
}

impl ImportService {
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
            app_id: super::repository::DEFAULT_APP_ID.to_owned(),
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
            app_id: super::repository::DEFAULT_APP_ID.to_owned(),
            repository,
            preview_generator,
        }
    }

    pub async fn for_app(&self, app_id: &str) -> SlideResult<Self> {
        Ok(Self {
            app_id: app_id.to_owned(),
            repository: self
                .repository
                .for_app(app_id)
                .await
                .map_err(SlideTemplateError::internal)?
                .unwrap_or_else(|| Arc::clone(&self.repository)),
            preview_generator: Arc::clone(&self.preview_generator),
        })
    }

    /// Imports a PowerPoint package and stores the resulting compact canvas JSON.
    ///
    /// # Errors
    ///
    /// Returns an API error for invalid packages, unsafe/oversized archives, unsafe images,
    /// or persistence failures.
    pub async fn import(&self, source: &[u8], kind: TemplateKind) -> SlideResult<ImportResponse> {
        self.import_single(source, kind, false, None).await
    }
    pub async fn import_cancellable(
        &self,
        source: &[u8],
        kind: TemplateKind,
        cancellation: &RequestCancellation,
    ) -> SlideResult<ImportResponse> {
        self.import_single(source, kind, false, Some(cancellation))
            .await
    }
    /// Save the JSON, generated preview and pending work in one transaction.
    pub async fn queue_classified_import(
        &self,
        source: &[u8],
        kind: TemplateKind,
        cancellation: &RequestCancellation,
    ) -> SlideResult<ImportResponse> {
        self.import_single(source, kind, true, Some(cancellation))
            .await
    }
    pub async fn complete_processing(&self, result: &CompleteSlideProcessing) -> SlideResult<()> {
        self.repository
            .complete_slide_processing(result)
            .await
            .map_err(SlideTemplateError::internal)
    }
    async fn import_single(
        &self,
        source: &[u8],
        kind: TemplateKind,
        queue_classification: bool,
        cancellation: Option<&RequestCancellation>,
    ) -> SlideResult<ImportResponse> {
        let bytes = source.to_vec();
        let cancelled = cancellation.cloned();
        let preparer = self.clone();
        let (record, result) = blocking(move || {
            let source = bytes.as_slice();
            let cancellation = cancelled.as_ref();
            let Conversion {
                template_json,
                warnings,
            } = pptx::read(source)?;
            if template_json.presentation.slides.len() != 1 {
                return Err(SlideTemplateError::new(
                    FailureKind::Invalid,
                    "template_must_have_one_slide",
                    "Template PowerPoint files must contain exactly one slide.",
                ));
            }
            let (record, result) = preparer.prepare_import(
                source,
                template_json,
                warnings,
                kind,
                queue_classification,
                cancellation,
            )?;

            Ok((record, result))
        })
        .await?;
        check_cancelled(cancellation)?;
        if queue_classification {
            let pending = pending_classification(&record)?;
            self.repository
                .insert_with_pending_classification_cancellable(&record, &pending, cancellation)
                .await
                .map_err(map_repository_failure)?;
        } else {
            self.repository
                .insert_many_cancellable(&[record], cancellation)
                .await
                .map_err(map_repository_failure)?;
        }
        Ok(result)
    }

    fn prepare_import(
        &self,
        source: &[u8],
        mut template_json: CanvasDocument,
        mut warnings: Vec<String>,
        kind: TemplateKind,
        require_preview: bool,
        cancellation: Option<&RequestCancellation>,
    ) -> SlideResult<(TemplateInsert, ImportResponse)> {
        check_cancelled(cancellation)?;
        repair_canvas_dimensions(&mut template_json);
        let generated_preview = match self.preview_generator.generate_document_cancellable(
            source,
            &template_json,
            cancellation,
        ) {
            Ok(preview) => preview,
            Err(error) => {
                let _ = error;
                tracing::warn!("template preview generation failed");
                None
            }
        };
        if require_preview && generated_preview.is_none() {
            return Err(super::facade::unavailable(
                "template_preview_unavailable",
                "A preview could not be generated. Try importing the template again.",
            ));
        }
        if generated_preview.is_none() {
            warnings.push("A preview image could not be generated for this template.".to_owned());
        }
        repair_canvas_dimensions(&mut template_json);
        let template_id = Uuid::new_v4().to_string();
        let (template_json, assets) = externalize_assets(&template_id, template_json)?;
        let template = StoredTemplate {
            template_id: template_id.clone(),
            template_json,
        };
        let metadata = StoredTemplateMetadata {
            template_id: template_id.clone(),
            kind,
            title: template.template_json.presentation.title.clone(),
            description: "Imported PowerPoint template".to_owned(),
            source: TemplateSource::Import,
            created_at: current_unix_millis(),
            catalog_version: None,
            catalog_checksum: None,
        };
        let preview = generated_preview.map(|preview| StoredTemplatePreview {
            template_id: template_id.clone(),
            content_type: "image/png".to_owned(),
            bytes: preview.bytes,
            width: preview.width,
            height: preview.height,
        });
        let hydrated_template_json =
            hydrate_canvas_assets(template.template_json.clone(), &assets)?;
        super::facade::bound_json(&super::compact_document(&hydrated_template_json))?;
        check_cancelled(cancellation)?;
        let preview_status = if preview.is_some() {
            TemplatePreviewStatus::Ready
        } else {
            TemplatePreviewStatus::Unavailable
        };
        Ok((
            TemplateInsert {
                template,
                assets,
                metadata,
                preview,
            },
            ImportResponse {
                template_id,
                template_json: hydrated_template_json,
                warnings,
                preview_status,
            },
        ))
    }

    /// Prepare slides concurrently, then commit the entire deck and durable jobs atomically.
    pub async fn queue_classified_batch_import(
        &self,
        source: &[u8],
        kind: TemplateKind,
        cancellation: &RequestCancellation,
    ) -> SlideResult<BatchImportResponse> {
        let bytes = source.to_vec();
        let cancelled = Some(cancellation.clone());
        let preparer = self.clone();
        let (records, response) = blocking(move || {
            let source = bytes.as_slice();
            let cancellation = cancelled
                .as_ref()
                .expect("required cancellation was cloned");
            let Conversion {
                template_json,
                warnings,
            } = pptx::read_parallel(source, cancellation)?;
            let presentation = template_json.presentation;
            let preserve_element_order = presentation.preserve_element_order;
            let show_branding = presentation.show_branding;
            let prepared = super::parallel::map(presentation.slides, |slide| {
                check_cancelled(Some(cancellation))?;
                let document = CanvasDocument {
                    presentation: Presentation {
                        title: slide.name.clone(),
                        preserve_element_order,
                        show_branding,
                        slides: vec![slide],
                    },
                };
                // Source-only preview adapters must receive a single-slide package.
                let single_source;
                let preview_source = if preparer.preview_generator.supports_independent_slides() {
                    source
                } else {
                    single_source = pptx::write(&document).map_err(SlideTemplateError::internal)?;
                    &single_source
                };
                preparer.prepare_import(
                    preview_source,
                    document,
                    Vec::new(),
                    kind,
                    true,
                    Some(cancellation),
                )
            })?;
            let mut records = Vec::with_capacity(prepared.len());
            let mut templates = Vec::with_capacity(prepared.len());
            let mut response = BatchImportResponse {
                templates: Vec::new(),
                warnings,
            };
            for (record, imported) in prepared {
                let pending = pending_classification(&record)?;
                records.push((record, pending));
                templates.push(BatchImportTemplate {
                    template_id: imported.template_id,
                    template_json: imported.template_json,
                    preview_available: true,
                });
            }
            response.templates = templates;
            // Bound the complete wire response before committing, including repeated IDs/metadata.
            super::facade::bound_json(&classified_batch_json(&response)?)?;

            Ok((records, response))
        })
        .await?;
        check_cancelled(Some(cancellation))?;
        self.repository
            .insert_many_with_pending_classification_cancellable(&records, Some(cancellation))
            .await
            .map_err(map_repository_failure)?;
        Ok(response)
    }

    /// Imports every PowerPoint slide as an independently stored template in one transaction.
    ///
    /// # Errors
    ///
    /// Returns an API error for invalid packages, unsafe content, or persistence failures. No
    /// template from the batch remains stored if any record cannot be inserted.
    pub async fn batch_import(
        &self,
        source: &[u8],
        kind: TemplateKind,
    ) -> SlideResult<BatchImportResponse> {
        self.batch_import_inner(source, kind, None).await
    }
    pub async fn batch_import_cancellable(
        &self,
        source: &[u8],
        kind: TemplateKind,
        cancellation: &RequestCancellation,
    ) -> SlideResult<BatchImportResponse> {
        self.batch_import_inner(source, kind, Some(cancellation))
            .await
    }
    async fn batch_import_inner(
        &self,
        source: &[u8],
        kind: TemplateKind,
        cancellation: Option<&RequestCancellation>,
    ) -> SlideResult<BatchImportResponse> {
        let bytes = source.to_vec();
        let cancelled = cancellation.cloned();
        let preparer = self.clone();
        let (records, templates, warnings) = blocking(move || {
            let source = bytes.as_slice();
            let cancellation = cancelled.as_ref();
            let Conversion {
                template_json,
                mut warnings,
            } = pptx::read(source)?;
            let presentation = template_json.presentation;
            if presentation.slides.is_empty() {
                return Err(SlideTemplateError::new(
                    FailureKind::Invalid,
                    "powerpoint_has_no_slides",
                    "The PowerPoint file must contain at least one slide.",
                ));
            }

            let created_at = current_unix_millis();
            let mut records = Vec::with_capacity(presentation.slides.len());
            let mut templates = Vec::with_capacity(presentation.slides.len());
            let mut response_bytes = 0usize;
            for (index, slide) in presentation.slides.into_iter().enumerate() {
                check_cancelled(cancellation)?;
                let mut template_json = CanvasDocument {
                    presentation: Presentation {
                        title: slide.name.clone(),
                        preserve_element_order: presentation.preserve_element_order,
                        show_branding: presentation.show_branding,
                        slides: vec![slide],
                    },
                };
                repair_canvas_dimensions(&mut template_json);
                let generated_preview =
                    if index > 0 && !preparer.preview_generator.supports_independent_slides() {
                        None
                    } else {
                        preparer
                            .preview_generator
                            .generate_document_cancellable(source, &template_json, cancellation)
                            .unwrap_or(None)
                    };
                check_cancelled(cancellation)?;
                if generated_preview.is_none() {
                    warnings.push(format!(
                        "Slide {}: A preview image could not be generated for this template.",
                        index + 1
                    ));
                }

                let template_id = Uuid::new_v4().to_string();
                let (stored_json, assets) = externalize_assets(&template_id, template_json)?;
                let hydrated_json = hydrate_canvas_assets(stored_json.clone(), &assets)?;
                response_bytes = response_bytes.saturating_add(super::facade::json_size(
                    &super::compact_document(&hydrated_json),
                )?);
                if response_bytes > super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES {
                    return Err(document_too_large());
                }
                let preview = generated_preview.map(|preview| StoredTemplatePreview {
                    template_id: template_id.clone(),
                    content_type: "image/png".to_owned(),
                    bytes: preview.bytes,
                    width: preview.width,
                    height: preview.height,
                });
                let preview_available = preview.is_some();
                records.push(TemplateInsert {
                    template: StoredTemplate {
                        template_id: template_id.clone(),
                        template_json: stored_json,
                    },
                    assets,
                    metadata: StoredTemplateMetadata {
                        template_id: template_id.clone(),
                        kind,
                        title: hydrated_json.presentation.title.clone(),
                        description: "Imported PowerPoint template".to_owned(),
                        source: TemplateSource::Import,
                        created_at,
                        catalog_version: None,
                        catalog_checksum: None,
                    },
                    preview,
                });
                templates.push(BatchImportTemplate {
                    template_id,
                    preview_available,
                    template_json: hydrated_json,
                });
            }

            Ok((records, templates, warnings))
        })
        .await?;
        check_cancelled(cancellation)?;
        self.repository
            .insert_many_cancellable(&records, cancellation)
            .await
            .map_err(map_repository_failure)?;
        Ok(BatchImportResponse {
            templates,
            warnings,
        })
    }

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
        let (mut template_json, migrated_assets) =
            externalize_assets(template_id, stored.template_json)?;
        let repaired_dimensions = repair_canvas_dimensions(&mut template_json);
        let mut assets = stored.assets;
        if repaired_dimensions || !migrated_assets.is_empty() {
            self.repository
                .update(
                    &StoredTemplate {
                        template_id: template_id.to_owned(),
                        template_json: template_json.clone(),
                    },
                    &migrated_assets,
                )
                .await
                .map_err(SlideTemplateError::internal)?;
            assets.extend(migrated_assets);
        }
        Ok(Some(TemplateResponse {
            template_id: template_id.to_owned(),
            template_json: hydrate_canvas_assets(template_json, &assets)?,
        }))
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
        Ok(Some(if builtin {
            super::normalized_document(&template.template_json)
        } else {
            super::compact_document(&template.template_json)
        }))
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
                            "/api/v1/templates/slide-templates/catalog/{}/preview?appId={}",
                            entry.template.template_id, self.app_id
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
        let total_pages = stored.total.div_ceil(TEMPLATE_PREVIEW_PAGE_SIZE);
        let encoded_size = stored
            .previews
            .iter()
            .map(|p| p.bytes.len().saturating_mul(4).div_ceil(3) + 128)
            .fold(0usize, usize::saturating_add);
        if encoded_size > super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES {
            return Err(document_too_large());
        }
        let previews = stored
            .previews
            .into_iter()
            .map(|preview| TemplatePreviewListItem {
                preview_url: format!(
                    "/api/v1/templates/slide-templates/catalog/{}/preview?appId={}",
                    preview.template_id, self.app_id
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
    }

    /// Deletes a stored template and its associated assets.
    pub async fn delete(&self, template_id: &str) -> SlideResult<bool> {
        self.repository
            .delete(template_id)
            .await
            .map_err(SlideTemplateError::internal)
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

    pub(crate) async fn projection_record(
        &self,
        id: &str,
    ) -> SlideResult<Option<super::graph::TemplateProjectionRecord>> {
        let Some(template) = self
            .repository
            .find_by_id(id)
            .await
            .map_err(SlideTemplateError::internal)?
        else {
            return Ok(None);
        };
        let metadata = self
            .repository
            .find_metadata(id)
            .await
            .map_err(SlideTemplateError::internal)?
            .ok_or_else(|| {
                SlideTemplateError::internal(anyhow::anyhow!("missing template metadata"))
            })?;
        let classification = self
            .repository
            .find_slide_classification(id)
            .await
            .map_err(SlideTemplateError::internal)?;
        let preview = self
            .repository
            .find_preview(id)
            .await
            .map_err(SlideTemplateError::internal)?
            .map(|p| super::graph::TemplatePreviewDescriptor {
                content_type: p.content_type,
                width: p.width,
                height: p.height,
                url: format!(
                    "/api/v1/templates/slide-templates/catalog/{id}/preview?appId={}",
                    self.app_id
                ),
            });
        Ok(Some(super::graph::TemplateProjectionRecord {
            template,
            metadata,
            classification,
            preview,
        }))
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
    /// The caller must have already selected an app-scoped service, so checking the template
    /// first prevents classification data from being exposed across app boundaries.
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

fn current_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}

fn inserted_file_name(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for character in value.chars() {
        let character = if character.is_ascii_alphanumeric()
            || matches!(character, '.' | '_' | ' ' | '(' | ')' | '-')
        {
            character
        } else {
            '-'
        };
        if character != '-' || !sanitized.ends_with('-') {
            sanitized.push(character);
        }
    }
    let trimmed = sanitized.trim();
    let stem = if trimmed.to_ascii_lowercase().ends_with(".pptx") {
        &trimmed[..trimmed.len().saturating_sub(5)]
    } else {
        trimmed
    };
    format!(
        "{}-with-slide.pptx",
        if stem.is_empty() {
            "presentation"
        } else {
            stem
        }
    )
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

fn assert_bounded_json(input: &Value) -> SlideResult<()> {
    let mut pending = vec![(0_usize, input)];
    let mut node_count = 0_usize;
    while let Some((depth, value)) = pending.pop() {
        node_count = node_count.saturating_add(1);
        if node_count > MAX_JSON_NODES || depth > MAX_JSON_DEPTH {
            return Err(SlideTemplateError::new(
                FailureKind::Invalid,
                "presentation_too_complex",
                "The presentation JSON exceeds the processing complexity limit.",
            ));
        }
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|value| (depth + 1, value))),
            Value::Object(values) => {
                pending.extend(values.values().map(|value| (depth + 1, value)));
            }
            Value::Number(number) if !number.as_f64().is_some_and(f64::is_finite) => {
                return Err(SlideTemplateError::new(
                    FailureKind::Invalid,
                    "invalid_json_value",
                    "The request body contains an unsupported JSON value.",
                ));
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    Ok(())
}

fn suggested_file_name(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut needs_separator = false;
    for character in title.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            if needs_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            needs_separator = false;
        } else {
            needs_separator = true;
        }
    }
    if slug.is_empty() {
        slug.push_str("generated-presentation");
    }
    slug.truncate(180);
    slug.push_str(".pptx");
    slug
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

fn pending_classification(record: &TemplateInsert) -> SlideResult<PendingSlideClassification> {
    let checksum = record
        .preview
        .as_ref()
        .map(|preview| checksum_bytes(&preview.bytes));
    Ok(PendingSlideClassification {
        input_fingerprint: build_slide_classification_input_fingerprint(
            &record.template.template_json,
            checksum.as_deref(),
        )
        .map_err(SlideTemplateError::internal)?,
        prompt_version: SLIDE_CLASSIFICATION_PROMPT_VERSION.into(),
        schema_version: SLIDE_CLASSIFICATION_SCHEMA_VERSION,
    })
}

pub(super) fn classified_batch_json(result: &BatchImportResponse) -> SlideResult<Value> {
    let templates = result
        .templates
        .iter()
        .map(|template| {
            serde_json::json!({
                "templateId": template.template_id,
                "templateJson": super::compact_document(&template.template_json),
                "previewAvailable": template.preview_available,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "templates": templates,
        "warnings": result.warnings,
        "importedCount": result.templates.len(),
        "retrieval": {"status": "pending", "embeddingStatus": "not_ready"},
    }))
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> SlideResult<T> + Send + 'static,
) -> SlideResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(SlideTemplateError::internal)?
}
