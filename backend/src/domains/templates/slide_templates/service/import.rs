//! Single-slide/deck preparation and atomic import publication.
use super::super::{
    check_cancelled, pptx, retrieval::*, BatchImportResponse, BatchImportTemplate, CanvasDocument,
    Conversion, FailureKind, ImportResponse, Presentation, RequestCancellation, SlideResult,
    SlideTemplateError, StoredTemplate, StoredTemplateMetadata, StoredTemplatePreview,
    TemplateInsert, TemplateKind, TemplatePreviewStatus, TemplateSource,
};
use super::assets::{externalize_assets, hydrate_canvas_assets};
use super::{
    blocking, document_too_large, map_repository_failure, repair_canvas_dimensions, ImportService,
};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Build the canonical record and the hydrated response from the same asset set.
/// Callers own preview policy, warnings, cancellation, and response-size bounds.
fn prepare_import_record(
    template_json: CanvasDocument,
    generated_preview: Option<super::super::preview::GeneratedTemplatePreview>,
    kind: TemplateKind,
    created_at: i64,
) -> SlideResult<(TemplateInsert, CanvasDocument)> {
    let template_id = Uuid::new_v4().to_string();
    let (template_json, assets) = externalize_assets(&template_id, template_json)?;
    // Storage retains asset references; the response owns a hydrated document.
    let hydrated_json = hydrate_canvas_assets(template_json.clone(), &assets)?;
    let metadata = StoredTemplateMetadata {
        template_id: template_id.clone(),
        kind,
        title: template_json.presentation.title.clone(),
        description: "Imported PowerPoint template".to_owned(),
        source: TemplateSource::Import,
        created_at,
        catalog_version: None,
        catalog_checksum: None,
    };
    let preview = generated_preview.map(|preview| StoredTemplatePreview {
        revision: 1,
        template_id: template_id.clone(),
        content_type: "image/png".to_owned(),
        bytes: preview.bytes,
        width: preview.width,
        height: preview.height,
    });
    Ok((
        TemplateInsert {
            template: StoredTemplate {
                revision: 1,
                template_id,
                template_json,
            },
            assets,
            metadata,
            preview,
        },
        hydrated_json,
    ))
}

impl ImportService {
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
            return Err(super::super::facade::unavailable(
                "template_preview_unavailable",
                "A preview could not be generated. Try importing the template again.",
            ));
        }
        if generated_preview.is_none() {
            warnings.push("A preview image could not be generated for this template.".to_owned());
        }
        repair_canvas_dimensions(&mut template_json);
        let (record, hydrated_template_json) = prepare_import_record(
            template_json,
            generated_preview,
            kind,
            current_unix_millis(),
        )?;
        super::super::facade::bound_json(&super::super::compact_document(&hydrated_template_json))?;
        check_cancelled(cancellation)?;
        let template_id = record.template.template_id.clone();
        let preview_status = if record.preview.is_some() {
            TemplatePreviewStatus::Ready
        } else {
            TemplatePreviewStatus::Unavailable
        };
        Ok((
            record,
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
        source: Vec<u8>,
        kind: TemplateKind,
        cancellation: &RequestCancellation,
    ) -> SlideResult<BatchImportResponse> {
        let cancelled = Some(cancellation.clone());
        let preparer = self.clone();
        let (records, response) = blocking(move || {
            let source = source.as_slice();
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
            let prepared = super::super::parallel::map(presentation.slides, |slide| {
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
            super::super::facade::bound_json(&classified_batch_json(&response)?)?;

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

                let (record, hydrated_json) =
                    prepare_import_record(template_json, generated_preview, kind, created_at)?;
                response_bytes = response_bytes.saturating_add(super::super::facade::json_size(
                    &super::super::compact_document(&hydrated_json),
                )?);
                if response_bytes > super::super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES {
                    return Err(document_too_large());
                }
                let preview_available = record.preview.is_some();
                let template_id = record.template.template_id.clone();
                records.push(record);
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
}
fn current_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
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

pub(in crate::domains::templates::slide_templates) fn classified_batch_json(
    result: &BatchImportResponse,
) -> SlideResult<Value> {
    let templates = result
        .templates
        .iter()
        .map(|template| {
            serde_json::json!({
                "templateId": template.template_id,
                "templateJson": super::super::compact_document(&template.template_json),
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
