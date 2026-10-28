//! Template-owned persistence through Quarry's bound query client.
mod apps;
mod classification;
use super::{compact_document, models::*, normalized_document, retrieval::*};
use crate::adapters::sqlite::{
    client::{SqliteClient, SqliteTransaction},
    query::{Condition, ConflictUpdate, JoinClause, JoinCondition, SortDirection, SqlBuilder},
};
use crate::shared::error::RepositoryError;
use anyhow::{ensure, Context};
pub use apps::{AppRegistration, StoredApp};
use std::sync::Arc;
pub type RepositoryResult<T> = Result<T, RepositoryError>;
pub const DEFAULT_APP_ID: &str = "Quarry_WestMonroe";
const MAX_STORED_PREVIEW_BYTES: usize = 16 * 1024 * 1024;
const MAX_STORED_PREVIEW_DIMENSION: u32 = 10_000;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
/// Persistence boundary used by both import and export services.
pub trait TemplateRepository: Send + Sync {
    fn insert_with_pending_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()>;
    fn insert_with_processing_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()>;
    fn find_slide_classification(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredSlideClassification>>;
    fn claim_next_slide_classification(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideClassificationJob>>;
    fn claim_next_slide_embedding(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideEmbeddingJob>>;
    fn complete_slide_classification(
        &self,
        result: &CompleteSlideClassification,
    ) -> RepositoryResult<()>;
    fn complete_slide_processing(&self, result: &CompleteSlideProcessing) -> RepositoryResult<()>;
    fn complete_slide_embedding(&self, result: &CompleteSlideEmbedding) -> RepositoryResult<()>;
    fn record_slide_classification_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()>;
    fn record_slide_embedding_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()>;
    fn refresh_pending_slide_classification(
        &self,
        id: &str,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<bool>;
    fn refresh_pending_slide_embedding(
        &self,
        id: &str,
        subject_document: &str,
        subject_fingerprint: &str,
        capability_document: &str,
        capability_fingerprint: &str,
    ) -> RepositoryResult<bool>;
    fn list_retrieval_classifications(
        &self,
        app_id: &str,
    ) -> RepositoryResult<Vec<RetrievalClassificationRecord>>;
    fn search_slide_classifications(
        &self,
        app_id: &str,
        fts_query: &str,
        limit: usize,
    ) -> RepositoryResult<Vec<String>>;
    /// Returns an independent, immutable app scope sharing the same durable store.
    fn for_app(&self, app_id: &str) -> RepositoryResult<Option<Arc<dyn TemplateRepository>>> {
        if app_id != DEFAULT_APP_ID {
            return Err(RepositoryError::invalid(
                "this repository does not implement app isolation",
            ));
        }
        Ok(None)
    }

    fn ensure_app(
        &self,
        app_id: &str,
        registration: Option<&AppRegistration>,
    ) -> RepositoryResult<StoredApp>;
    fn find_app(&self, app_id: &str) -> RepositoryResult<Option<StoredApp>>;
    /// Inserts a template, metadata, assets, and optional preview atomically.
    fn insert(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()>;
    fn insert_many_cancellable(
        &self,
        records: &[TemplateInsert],
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        check_publication(cancellation)?;
        self.insert_many(records)
    }
    fn insert_with_processing_classification_cancellable(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        check_publication(cancellation)?;
        self.insert_with_processing_classification(record, pending)
    }
    fn complete_slide_processing_cancellable(
        &self,
        result: &CompleteSlideProcessing,
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        check_publication(cancellation)?;
        self.complete_slide_processing(result)
    }
    /// Inserts a group of templates and their related records atomically.
    fn insert_many(&self, records: &[TemplateInsert]) -> RepositoryResult<()>;
    /// Updates a template and adds newly externalized assets atomically.
    fn update(&self, template: &StoredTemplate, assets: &[TemplateAsset]) -> RepositoryResult<()>;
    /// Inserts or refreshes a server-owned built-in template.
    fn upsert_builtin(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()>;
    /// Finds a template by its public identifier.
    fn find_by_id(&self, template_id: &str) -> RepositoryResult<Option<StoredTemplate>>;
    /// Finds metadata by template identifier.
    fn find_metadata(&self, template_id: &str) -> RepositoryResult<Option<StoredTemplateMetadata>>;
    /// Finds a stored first-slide preview.
    fn find_preview(&self, template_id: &str) -> RepositoryResult<Option<StoredTemplatePreview>>;
    /// Lists stored previews from newest to oldest without loading template JSON.
    fn list_previews(
        &self,
        limit: usize,
        offset: usize,
    ) -> RepositoryResult<StoredTemplatePreviewPage>;
    /// Lists catalog entries from newest to oldest without loading preview BLOBs.
    fn list(&self, kind: Option<TemplateKind>) -> RepositoryResult<Vec<StoredTemplateCatalogItem>>;
    /// Deletes a template and its associated records, returning whether it existed.
    fn delete(&self, template_id: &str) -> RepositoryResult<bool>;
    /// Finds a template and all of its joined image BLOBs.
    fn find_by_id_with_assets(
        &self,
        template_id: &str,
    ) -> RepositoryResult<Option<StoredTemplateWithAssets>>;
    /// Finds an asset only when it belongs to the requested template.
    fn find_asset(
        &self,
        template_id: &str,
        asset_id: &str,
    ) -> RepositoryResult<Option<TemplateAsset>>;
}

#[derive(Clone)]
pub struct SqliteTemplateRepository {
    client: SqliteClient,
    app_id: String,
}
impl SqliteTemplateRepository {
    pub fn new(client: SqliteClient) -> Self {
        Self {
            client,
            app_id: DEFAULT_APP_ID.into(),
        }
    }
}
impl TemplateRepository for SqliteTemplateRepository {
    fn insert_with_pending_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::insert_with_pending_classification(self, record, pending)
        })
    }
    fn insert_with_processing_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::insert_with_processing_classification(self, record, pending)
        })
    }
    fn find_slide_classification(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredSlideClassification>> {
        repository_boundary(|| -> anyhow::Result<_> { Self::find_slide_classification(self, id) })
    }
    fn claim_next_slide_classification(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideClassificationJob>> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::claim_next_slide_classification(self, now, lease_expires_at)
        })
    }
    fn claim_next_slide_embedding(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideEmbeddingJob>> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::claim_next_slide_embedding(self, now, lease_expires_at)
        })
    }
    fn complete_slide_classification(
        &self,
        result: &CompleteSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::complete_slide_classification(self, result)
        })
    }
    fn complete_slide_processing(&self, result: &CompleteSlideProcessing) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::complete_slide_processing(self, result)
        })
    }
    fn complete_slide_embedding(&self, result: &CompleteSlideEmbedding) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::complete_slide_embedding(self, result)
        })
    }
    fn record_slide_classification_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::record_slide_classification_failure(self, id, code, retry)
        })
    }
    fn record_slide_embedding_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::record_slide_embedding_failure(self, id, code, retry)
        })
    }
    fn refresh_pending_slide_classification(
        &self,
        id: &str,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<bool> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::refresh_pending_slide_classification(self, id, pending)
        })
    }
    fn refresh_pending_slide_embedding(
        &self,
        id: &str,
        subject_document: &str,
        subject_fingerprint: &str,
        capability_document: &str,
        capability_fingerprint: &str,
    ) -> RepositoryResult<bool> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::refresh_pending_slide_embedding(
                self,
                id,
                subject_document,
                subject_fingerprint,
                capability_document,
                capability_fingerprint,
            )
        })
    }
    fn list_retrieval_classifications(
        &self,
        app_id: &str,
    ) -> RepositoryResult<Vec<RetrievalClassificationRecord>> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::list_retrieval_classifications(self, app_id)
        })
    }
    fn search_slide_classifications(
        &self,
        app_id: &str,
        fts_query: &str,
        limit: usize,
    ) -> RepositoryResult<Vec<String>> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::search_slide_classifications(self, app_id, fts_query, limit)
        })
    }
    fn ensure_app(
        &self,
        app_id: &str,
        registration: Option<&AppRegistration>,
    ) -> RepositoryResult<StoredApp> {
        repository_boundary(|| -> anyhow::Result<_> {
            Self::ensure_app(self, app_id, registration)
        })
    }
    fn find_app(&self, app_id: &str) -> RepositoryResult<Option<StoredApp>> {
        repository_boundary(|| -> anyhow::Result<_> { Self::find_app(self, app_id) })
    }

    fn for_app(&self, app_id: &str) -> RepositoryResult<Option<Arc<dyn TemplateRepository>>> {
        repository_boundary(|| -> anyhow::Result<Option<Arc<dyn TemplateRepository>>> {
            self.ensure_app(app_id, None)?;
            Ok(Some(Arc::new(Self {
                client: self.client.clone(),
                app_id: app_id.into(),
            })))
        })
    }
    fn insert(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Ok(self.insert_many(&[TemplateInsert {
                template: template.clone(),
                assets: assets.to_vec(),
                metadata: metadata.clone(),
                preview: preview.cloned(),
            }])?)
        })
    }
    fn insert_many(&self, records: &[TemplateInsert]) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            Ok(self.insert_many_cancellable(records, None)?)
        })
    }
    fn insert_many_cancellable(
        &self,
        records: &[TemplateInsert],
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            for record in records {
                validate_records(
                    &record.template,
                    &record.assets,
                    &record.metadata,
                    record.preview.as_ref(),
                )?;
            }
            self.client.transaction_result(|tx| -> anyhow::Result<()> {
                check_publication(cancellation)?;
                for record in records {
                    Self::write_new(
                        tx,
                        &record.template,
                        &record.assets,
                        &record.metadata,
                        record.preview.as_ref(),
                    )?;
                    membership(tx, &self.app_id, &record.template.template_id)?;
                }
                check_publication(cancellation)
            })
        })
    }
    fn insert_with_processing_classification_cancellable(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.insert_classification(record, pending, true, cancellation)
        })
    }
    fn complete_slide_processing_cancellable(
        &self,
        result: &CompleteSlideProcessing,
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.publish(&result.classification, Some(result), cancellation)
        })
    }
    fn update(&self, template: &StoredTemplate, assets: &[TemplateAsset]) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            ensure!(
                assets.iter().all(|a| a.template_id == template.template_id),
                "asset ownership mismatch"
            );
            self.client.transaction_result(|tx| -> anyhow::Result<()> {
                ensure!(
                    visible(tx, &self.app_id, &template.template_id)?,
                    "template is unavailable to this app"
                );
                let builtin = tx
                    .read_one_with(
                        SqlBuilder::select("template_metadata")
                            .columns(["source"])
                            .and_where(Condition::equal("template_id", &template.template_id))
                            .build(),
                        |r| r.get::<_, String>(0),
                    )?
                    .is_some_and(|s| s == "builtin");
                let json = serde_json::to_string(&if builtin {
                    normalized_document(&template.template_json)
                } else {
                    compact_document(&template.template_json)
                })?;
                tx.write(
                    SqlBuilder::update("templates")
                        .set("template_json", json)
                        .and_where(Condition::equal("template_id", &template.template_id))
                        .build(),
                )?;
                insert_assets(tx, assets)?;
                Ok(())
            })
        })
    }
    fn upsert_builtin(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()> {
        repository_boundary(|| -> anyhow::Result<_> {
            validate_records(template, assets, metadata, preview)?;
            self.client.transaction_result(|tx| -> anyhow::Result<()> {
                let existing = tx.read_one_with(
                    SqlBuilder::select("template_metadata")
                        .columns(["source", "checksum"])
                        .and_where(Condition::equal("template_id", &template.template_id))
                        .build(),
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
                )?;
                if existing.is_some_and(|(s, c)| s == "import" || c == metadata.catalog_checksum) {
                    return Ok(());
                }
                let json = serde_json::to_string(&normalized_document(&template.template_json))?;
                tx.write(
                    SqlBuilder::insert_into("templates")
                        .value("template_id", &template.template_id)
                        .value("template_json", json)
                        .on_conflict_update(
                            ConflictUpdate::new(["template_id"]).set_excluded("template_json"),
                        )
                        .build(),
                )?;
                for table in ["template_assets", "template_previews"] {
                    tx.write(
                        SqlBuilder::delete_from(table)
                            .and_where(Condition::equal("template_id", &template.template_id))
                            .build(),
                    )?;
                }
                write_metadata(tx, metadata, true)?;
                insert_assets(tx, assets)?;
                write_preview(tx, preview)?;
                let apps = tx.read_with(
                    SqlBuilder::select("apps").columns(["app_id"]).build(),
                    |r| r.get::<_, String>(0),
                )?;
                for app in apps {
                    membership(tx, &app, &template.template_id)?;
                }
                Ok(())
            })
        })
    }
    fn find_by_id(&self, id: &str) -> RepositoryResult<Option<StoredTemplate>> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                if !visible(tx, &self.app_id, id)? {
                    return Ok(None);
                }
                let json = tx.read_one_with(
                    SqlBuilder::select("templates")
                        .columns(["template_json"])
                        .and_where(Condition::equal("template_id", id))
                        .build(),
                    |r| r.get::<_, String>(0),
                )?;
                json.map(|json| {
                    Ok(StoredTemplate {
                        template_id: id.into(),
                        template_json: decode_document(&json)?,
                    })
                })
                .transpose()
            })
        })
    }
    fn find_metadata(&self, id: &str) -> RepositoryResult<Option<StoredTemplateMetadata>> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                if !visible(tx, &self.app_id, id)? {
                    return Ok(None);
                }
                metadata_in(tx, id)
            })
        })
    }
    fn find_preview(&self, id: &str) -> RepositoryResult<Option<StoredTemplatePreview>> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                if !visible(tx, &self.app_id, id)? {
                    return Ok(None);
                }
                Ok(tx.read_one_with(
                    SqlBuilder::select("template_previews")
                        .and_where(Condition::equal("template_id", id))
                        .build(),
                    map_preview,
                )?)
            })
        })
    }
    fn list_previews(
        &self,
        limit: usize,
        offset: usize,
    ) -> RepositoryResult<StoredTemplatePreviewPage> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                let query = SqlBuilder::select("template_previews")
                    .columns(["template_previews.*"])
                    .join(JoinClause::inner("app_templates").on(JoinCondition::equal(
                        "app_templates.template_id",
                        "template_previews.template_id",
                    )))
                    .join(JoinClause::inner("templates").on(JoinCondition::equal(
                        "templates.template_id",
                        "template_previews.template_id",
                    )))
                    .join(
                        JoinClause::inner("template_metadata").on(JoinCondition::equal(
                            "template_metadata.template_id",
                            "template_previews.template_id",
                        )),
                    )
                    .and_where(Condition::equal("app_templates.app_id", &self.app_id));
                let total = usize::try_from(
                    tx.read_one_with(query.clone().count_rows().build(), |r| r.get::<_, i64>(0))?
                        .unwrap_or(0),
                )?;
                let previews = tx.read_with(
                    query
                        .order_by("template_metadata.created_at", SortDirection::Descending)
                        .order_by("templates.rowid", SortDirection::Descending)
                        .limit(limit)
                        .offset(offset)
                        .build(),
                    map_preview,
                )?;
                Ok(StoredTemplatePreviewPage { previews, total })
            })
        })
    }
    fn list(&self, kind: Option<TemplateKind>) -> RepositoryResult<Vec<StoredTemplateCatalogItem>> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                let mut query = SqlBuilder::select("templates")
                    .columns(["templates.template_id", "templates.template_json"])
                    .join(
                        JoinClause::inner("template_metadata").on(JoinCondition::equal(
                            "template_metadata.template_id",
                            "templates.template_id",
                        )),
                    )
                    .join(JoinClause::inner("app_templates").on(JoinCondition::equal(
                        "app_templates.template_id",
                        "templates.template_id",
                    )))
                    .and_where(Condition::equal("app_templates.app_id", &self.app_id))
                    .order_by("template_metadata.created_at", SortDirection::Descending)
                    .order_by("templates.rowid", SortDirection::Descending);
                if let Some(kind) = kind {
                    query =
                        query.and_where(Condition::equal("template_metadata.kind", kind.as_str()));
                }
                let rows = tx.read_with(query.build(), |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?;
                rows.into_iter()
                    .map(|(id, json)| {
                        let metadata =
                            metadata_in(tx, &id)?.context("missing template metadata")?;
                        let has_preview = tx
                            .read_one(
                                SqlBuilder::select("template_previews")
                                    .columns(["template_id"])
                                    .and_where(Condition::equal("template_id", &id))
                                    .build(),
                            )?
                            .is_some();
                        Ok(StoredTemplateCatalogItem {
                            template: StoredTemplate {
                                template_id: id,
                                template_json: decode_document(&json)?,
                            },
                            metadata,
                            has_preview,
                        })
                    })
                    .collect()
            })
        })
    }
    fn delete(&self, id: &str) -> RepositoryResult<bool> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                let changed = tx
                    .write(
                        SqlBuilder::delete_from("app_templates")
                            .and_where(Condition::equal("app_id", &self.app_id))
                            .and_where(Condition::equal("template_id", id))
                            .build(),
                    )?
                    .rows_affected;
                // Keep the canonical built-in/checksum when its last membership is removed.
                // Reseeding can then distinguish a deletion choice from an unseeded template.
                let builtin = metadata_in(tx, id)?
                    .is_some_and(|metadata| metadata.source == TemplateSource::Builtin);
                if !builtin
                    && tx
                        .read_one(
                            SqlBuilder::select("app_templates")
                                .and_where(Condition::equal("template_id", id))
                                .limit(1)
                                .build(),
                        )?
                        .is_none()
                {
                    for table in ["slide_classifications_fts", "templates"] {
                        tx.write(
                            SqlBuilder::delete_from(table)
                                .and_where(Condition::equal("template_id", id))
                                .build(),
                        )?;
                    }
                }
                Ok(changed > 0)
            })
        })
    }
    fn find_by_id_with_assets(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredTemplateWithAssets>> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                if !visible(tx, &self.app_id, id)? {
                    return Ok(None);
                }
                let json = tx
                    .read_one_with(
                        SqlBuilder::select("templates")
                            .columns(["template_json"])
                            .and_where(Condition::equal("template_id", id))
                            .build(),
                        |r| r.get::<_, String>(0),
                    )?
                    .context("missing template")?;
                let assets = tx.read_with(
                    SqlBuilder::select("template_assets")
                        .and_where(Condition::equal("template_id", id))
                        .order_by("asset_id", SortDirection::Ascending)
                        .build(),
                    map_asset,
                )?;
                Ok(Some(StoredTemplateWithAssets {
                    template_id: id.into(),
                    template_json: decode_document(&json)?,
                    assets,
                }))
            })
        })
    }
    fn find_asset(&self, id: &str, asset: &str) -> RepositoryResult<Option<TemplateAsset>> {
        repository_boundary(|| -> anyhow::Result<_> {
            self.client.transaction_result(|tx| {
                if !visible(tx, &self.app_id, id)? {
                    return Ok(None);
                }
                Ok(tx.read_one_with(
                    SqlBuilder::select("template_assets")
                        .and_where(Condition::equal("template_id", id))
                        .and_where(Condition::equal("asset_id", asset))
                        .build(),
                    map_asset,
                )?)
            })
        })
    }
}
fn validate_records(
    template: &StoredTemplate,
    assets: &[TemplateAsset],
    metadata: &StoredTemplateMetadata,
    preview: Option<&StoredTemplatePreview>,
) -> anyhow::Result<()> {
    ensure!(
        metadata.template_id == template.template_id
            && preview.is_none_or(|preview| preview.template_id == template.template_id),
        "template records must use the same template ID"
    );
    ensure!(
        assets
            .iter()
            .all(|asset| asset.template_id == template.template_id),
        "a template asset cannot be stored under a different template"
    );
    if let Some(preview) = preview {
        ensure!(
            preview.content_type == "image/png"
                && preview.bytes.len() >= 24
                && preview.bytes.len() <= MAX_STORED_PREVIEW_BYTES
                && preview.bytes.starts_with(PNG_SIGNATURE),
            "template previews must contain a valid bounded PNG image"
        );
        let dimensions = png_dimensions(&preview.bytes)
            .context("template preview dimensions could not be read")?;
        ensure!(
            dimensions == (preview.width, preview.height)
                && preview.width <= MAX_STORED_PREVIEW_DIMENSION
                && preview.height <= MAX_STORED_PREVIEW_DIMENSION,
            "template preview dimensions do not match the PNG image"
        );
    }
    Ok(())
}

type MetadataRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
);

fn map_metadata(row: MetadataRow) -> anyhow::Result<StoredTemplateMetadata> {
    let kind = match row.1.as_str() {
        "diagram" => TemplateKind::Diagram,
        "commentary" => TemplateKind::Commentary,
        _ => anyhow::bail!("stored template kind is invalid"),
    };
    let source = match row.4.as_str() {
        "builtin" => TemplateSource::Builtin,
        "import" => TemplateSource::Import,
        _ => anyhow::bail!("stored template source is invalid"),
    };
    Ok(StoredTemplateMetadata {
        template_id: row.0,
        kind,
        title: row.2,
        description: row.3,
        source,
        created_at: timestamp_millis(&row.5)?,
        catalog_version: row.6,
        catalog_checksum: row.7,
    })
}

pub(super) fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(PNG_SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

fn decode_document(json: &str) -> anyhow::Result<CanvasDocument> {
    let value = serde_json::from_str(json)?;
    Ok(super::normalize::normalize(&value)?.0)
}

fn timestamp_iso(millis: i64) -> anyhow::Result<String> {
    Ok(chrono::DateTime::from_timestamp_millis(millis)
        .context("invalid creation time")?
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}
fn timestamp_millis(value: &str) -> anyhow::Result<i64> {
    if let Ok(time) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(time.timestamp_millis());
    }
    Ok(
        chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")?
            .and_utc()
            .timestamp_millis(),
    )
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/templates/slide_templates/repository_tests.rs"]
mod tests;
impl SqliteTemplateRepository {
    fn write_new(
        tx: &SqliteTransaction<'_>,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> anyhow::Result<()> {
        validate_records(template, assets, metadata, preview)?;
        let json = serde_json::to_string(&if metadata.source == TemplateSource::Builtin {
            normalized_document(&template.template_json)
        } else {
            compact_document(&template.template_json)
        })?;
        tx.write(
            SqlBuilder::insert_into("templates")
                .value("template_id", &template.template_id)
                .value("template_json", json)
                .build(),
        )?;
        write_metadata(tx, metadata, false)?;
        insert_assets(tx, assets)?;
        write_preview(tx, preview)
    }
}
fn membership(tx: &SqliteTransaction<'_>, app: &str, id: &str) -> anyhow::Result<()> {
    tx.write(
        SqlBuilder::insert_into("app_templates")
            .value("app_id", app)
            .value("template_id", id)
            .value("created_at", chrono::Utc::now().to_rfc3339())
            .on_conflict_do_nothing(["app_id", "template_id"])
            .build(),
    )?;
    Ok(())
}
fn visible(tx: &SqliteTransaction<'_>, app: &str, id: &str) -> anyhow::Result<bool> {
    Ok(tx
        .read_one(
            SqlBuilder::select("app_templates")
                .columns(["template_id"])
                .and_where(Condition::equal("app_id", app))
                .and_where(Condition::equal("template_id", id))
                .build(),
        )?
        .is_some())
}
fn write_metadata(
    tx: &SqliteTransaction<'_>,
    m: &StoredTemplateMetadata,
    upsert: bool,
) -> anyhow::Result<()> {
    let mut q = SqlBuilder::insert_into("template_metadata")
        .value("template_id", &m.template_id)
        .value("kind", m.kind.as_str())
        .value("description", &m.description)
        .value("source", m.source.as_str())
        .value("checksum", m.catalog_checksum.as_deref())
        .value("created_at", timestamp_iso(m.created_at)?);
    if upsert {
        let mut conflict = ConflictUpdate::new(["template_id"]);
        for col in ["kind", "description", "source", "checksum", "created_at"] {
            conflict = conflict.set_excluded(col);
        }
        q = q.on_conflict_update(conflict);
    }
    tx.write(q.build())?;
    Ok(())
}
fn write_preview(
    tx: &SqliteTransaction<'_>,
    p: Option<&StoredTemplatePreview>,
) -> anyhow::Result<()> {
    if let Some(p) = p {
        tx.write(
            SqlBuilder::insert_into("template_previews")
                .value("template_id", &p.template_id)
                .value("content_type", &p.content_type)
                .value("preview_data", p.bytes.as_slice())
                .value("width", p.width)
                .value("height", p.height)
                .build(),
        )?;
    }
    Ok(())
}
fn insert_assets(tx: &SqliteTransaction<'_>, assets: &[TemplateAsset]) -> anyhow::Result<()> {
    for a in assets {
        tx.write(
            SqlBuilder::insert_into("template_assets")
                .value("asset_id", &a.asset_id)
                .value("template_id", &a.template_id)
                .value("content_type", &a.content_type)
                .value("asset_data", a.bytes.as_slice())
                .build(),
        )?;
    }
    Ok(())
}
fn map_asset(r: &rusqlite::Row<'_>) -> rusqlite::Result<TemplateAsset> {
    Ok(TemplateAsset {
        asset_id: r.get("asset_id")?,
        template_id: r.get("template_id")?,
        content_type: r.get("content_type")?,
        bytes: r.get("asset_data")?,
    })
}
fn map_preview(r: &rusqlite::Row<'_>) -> rusqlite::Result<StoredTemplatePreview> {
    Ok(StoredTemplatePreview {
        template_id: r.get("template_id")?,
        content_type: r.get("content_type")?,
        bytes: r.get("preview_data")?,
        width: r.get("width")?,
        height: r.get("height")?,
    })
}
fn metadata_in(
    tx: &SqliteTransaction<'_>,
    id: &str,
) -> anyhow::Result<Option<StoredTemplateMetadata>> {
    let row = tx.read_one_with(
        SqlBuilder::select("template_metadata")
            .and_where(Condition::equal("template_id", id))
            .build(),
        |r| {
            Ok((
                r.get::<_, String>("kind")?,
                r.get::<_, String>("description")?,
                r.get::<_, String>("source")?,
                r.get::<_, String>("created_at")?,
                r.get::<_, Option<String>>("checksum")?,
            ))
        },
    )?;
    row.map(|(kind, description, source, created, checksum)| {
        let json = tx
            .read_one_with(
                SqlBuilder::select("templates")
                    .columns(["template_json"])
                    .and_where(Condition::equal("template_id", id))
                    .build(),
                |r| r.get::<_, String>(0),
            )?
            .context("missing template")?;
        let title = serde_json::from_str::<serde_json::Value>(&json)?["presentation"]["title"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        map_metadata((
            id.into(),
            kind,
            title,
            description,
            source,
            created,
            None,
            checksum,
        ))
    })
    .transpose()
}

#[derive(Debug, thiserror::Error)]
#[error("template publication was cancelled")]
pub(super) struct PublicationCancelled;
fn check_publication(c: Option<&super::RequestCancellation>) -> anyhow::Result<()> {
    if c.is_some_and(super::RequestCancellation::is_cancelled) {
        return Err(PublicationCancelled.into());
    }
    Ok(())
}

fn repository_boundary<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> RepositoryResult<T> {
    operation().map_err(RepositoryError::from)
}
