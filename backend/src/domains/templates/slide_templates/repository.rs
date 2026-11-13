//! Template-owned persistence through Quarry's bound query client.
mod apps;
mod classification;
use super::{compact_document, models::*, normalized_document, retrieval::*};
use crate::adapters::postgres::client::RowResult;
use crate::adapters::postgres::{
    client::{PostgresClient, PostgresTransaction},
    query::{
        Condition, ConflictUpdate, JoinClause, JoinCondition, LockWait, RowLock, SortDirection,
        SqlBuilder,
    },
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
#[async_trait::async_trait]
pub trait TemplateRepository: Send + Sync {
    /// Whether this scope has unfinished stages, including delayed retries and leases.
    async fn has_pending_slide_processing(&self) -> RepositoryResult<bool>;
    /// Atomically reset only a terminal failed stage in this app's saved template.
    async fn retry_failed_slide_processing(
        &self,
        id: &str,
    ) -> RepositoryResult<RetrySlideProcessingOutcome>;
    /// Publish every slide and its pending job in the same transaction.
    async fn insert_many_with_pending_classification_cancellable(
        &self,
        records: &[(TemplateInsert, PendingSlideClassification)],
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()>;
    async fn insert_with_pending_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()>;
    async fn insert_with_processing_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()>;
    async fn find_slide_classification(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredSlideClassification>>;
    async fn claim_next_slide_classification(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideClassificationJob>>;
    async fn claim_next_slide_embedding(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideEmbeddingJob>>;
    async fn complete_slide_classification(
        &self,
        result: &CompleteSlideClassification,
    ) -> RepositoryResult<()>;
    async fn complete_slide_processing(
        &self,
        result: &CompleteSlideProcessing,
    ) -> RepositoryResult<()>;
    async fn complete_slide_embedding(
        &self,
        result: &CompleteSlideEmbedding,
    ) -> RepositoryResult<()>;
    async fn record_slide_classification_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()>;
    async fn record_slide_embedding_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()>;
    async fn refresh_pending_slide_classification(
        &self,
        id: &str,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<bool>;
    async fn refresh_pending_slide_embedding(
        &self,
        id: &str,
        subject_document: &str,
        subject_fingerprint: &str,
        capability_document: &str,
        capability_fingerprint: &str,
    ) -> RepositoryResult<bool>;
    async fn list_retrieval_classifications(
        &self,
        app_id: &str,
    ) -> RepositoryResult<Vec<RetrievalClassificationRecord>>;
    async fn search_slide_classifications(
        &self,
        app_id: &str,
        fts_query: &str,
        limit: usize,
    ) -> RepositoryResult<Vec<String>>;
    /// Immutable owner scope derived only from an authenticated server-side identity.
    fn for_user(&self, _user_id: i64) -> RepositoryResult<Arc<dyn TemplateRepository>> {
        Err(RepositoryError::invalid(
            "this repository does not implement user isolation",
        ))
    }

    /// Returns an independent, immutable app scope sharing the same durable store.
    async fn for_app(&self, app_id: &str) -> RepositoryResult<Option<Arc<dyn TemplateRepository>>> {
        if app_id != DEFAULT_APP_ID {
            return Err(RepositoryError::invalid(
                "this repository does not implement app isolation",
            ));
        }
        Ok(None)
    }

    async fn ensure_app(
        &self,
        app_id: &str,
        registration: Option<&AppRegistration>,
    ) -> RepositoryResult<StoredApp>;
    async fn find_app(&self, app_id: &str) -> RepositoryResult<Option<StoredApp>>;
    /// Inserts a template, metadata, assets, and optional preview atomically.
    async fn insert(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()>;
    async fn insert_many_cancellable(
        &self,
        records: &[TemplateInsert],
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        check_publication(cancellation)?;
        self.insert_many(records).await
    }
    async fn insert_with_pending_classification_cancellable(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        check_publication(cancellation)?;
        self.insert_with_pending_classification(record, pending)
            .await
    }

    /// Inserts a group of templates and their related records atomically.
    async fn insert_many(&self, records: &[TemplateInsert]) -> RepositoryResult<()>;
    /// Updates a template and adds newly externalized assets atomically.
    async fn update(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
    ) -> RepositoryResult<()>;
    /// Inserts or refreshes a server-owned built-in template.
    async fn upsert_builtin(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()>;
    /// Finds a template by its public identifier.
    async fn find_by_id(&self, template_id: &str) -> RepositoryResult<Option<StoredTemplate>>;
    /// Finds metadata by template identifier.
    async fn find_metadata(
        &self,
        template_id: &str,
    ) -> RepositoryResult<Option<StoredTemplateMetadata>>;
    /// Finds a stored first-slide preview.
    async fn find_preview(
        &self,
        template_id: &str,
    ) -> RepositoryResult<Option<StoredTemplatePreview>>;
    /// Lists stored previews from newest to oldest without loading template JSON.
    async fn list_previews(
        &self,
        limit: usize,
        offset: usize,
    ) -> RepositoryResult<StoredTemplatePreviewPage>;
    /// Lists catalog entries from newest to oldest without loading preview BLOBs.
    async fn list(
        &self,
        kind: Option<TemplateKind>,
    ) -> RepositoryResult<Vec<StoredTemplateCatalogItem>>;
    /// Deletes a template and its associated records, returning whether it existed.
    async fn delete(&self, template_id: &str, expected_revision: i64) -> RepositoryResult<bool>;
    /// Finds a template and all of its joined image BLOBs.
    async fn find_by_id_with_assets(
        &self,
        template_id: &str,
    ) -> RepositoryResult<Option<StoredTemplateWithAssets>>;
    /// Finds an asset only when it belongs to the requested template.
    async fn find_asset(
        &self,
        template_id: &str,
        asset_id: &str,
    ) -> RepositoryResult<Option<TemplateAsset>>;
}

#[derive(Clone)]
pub struct PostgresTemplateRepository {
    client: PostgresClient,
    app_id: String,
    owner_user_id: Option<i64>,
}
impl PostgresTemplateRepository {
    pub(crate) fn user_scope(&self, user_id: i64) -> RepositoryResult<Self> {
        if user_id <= 0 || self.owner_user_id.is_some_and(|owner| owner != user_id) {
            return Err(RepositoryError::Forbidden);
        }
        Ok(Self {
            client: self.client.clone(),
            app_id: self.app_id.clone(),
            owner_user_id: Some(user_id),
        })
    }

    pub fn new(client: PostgresClient) -> Self {
        Self {
            client,
            app_id: DEFAULT_APP_ID.into(),
            owner_user_id: None,
        }
    }
}
#[async_trait::async_trait]
impl TemplateRepository for PostgresTemplateRepository {
    async fn has_pending_slide_processing(&self) -> RepositoryResult<bool> {
        repository_boundary(async { Self::has_pending_slide_processing(self).await }).await
    }
    async fn retry_failed_slide_processing(
        &self,
        id: &str,
    ) -> RepositoryResult<RetrySlideProcessingOutcome> {
        repository_boundary(async { Self::retry_failed_slide_processing(self, id).await }).await
    }
    async fn insert_many_with_pending_classification_cancellable(
        &self,
        records: &[(TemplateInsert, PendingSlideClassification)],
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(async { self.insert_classifications(records, cancellation).await })
            .await
    }
    async fn insert_with_pending_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            Self::insert_with_pending_classification(self, record, pending).await
        })
        .await
    }
    async fn insert_with_processing_classification(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            Self::insert_with_processing_classification(self, record, pending).await
        })
        .await
    }
    async fn find_slide_classification(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredSlideClassification>> {
        repository_boundary(async { Self::find_slide_classification(self, id).await }).await
    }
    async fn claim_next_slide_classification(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideClassificationJob>> {
        repository_boundary(async {
            Self::claim_next_slide_classification(self, now, lease_expires_at).await
        })
        .await
    }
    async fn claim_next_slide_embedding(
        &self,
        now: &str,
        lease_expires_at: &str,
    ) -> RepositoryResult<Option<SlideEmbeddingJob>> {
        repository_boundary(async {
            Self::claim_next_slide_embedding(self, now, lease_expires_at).await
        })
        .await
    }
    async fn complete_slide_classification(
        &self,
        result: &CompleteSlideClassification,
    ) -> RepositoryResult<()> {
        repository_boundary(async { Self::complete_slide_classification(self, result).await }).await
    }
    async fn complete_slide_processing(
        &self,
        result: &CompleteSlideProcessing,
    ) -> RepositoryResult<()> {
        repository_boundary(async { Self::complete_slide_processing(self, result).await }).await
    }
    async fn complete_slide_embedding(
        &self,
        result: &CompleteSlideEmbedding,
    ) -> RepositoryResult<()> {
        repository_boundary(async { Self::complete_slide_embedding(self, result).await }).await
    }
    async fn record_slide_classification_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            Self::record_slide_classification_failure(self, id, code, retry).await
        })
        .await
    }
    async fn record_slide_embedding_failure(
        &self,
        id: &str,
        code: &str,
        retry: Option<&str>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            Self::record_slide_embedding_failure(self, id, code, retry).await
        })
        .await
    }
    async fn refresh_pending_slide_classification(
        &self,
        id: &str,
        pending: &PendingSlideClassification,
    ) -> RepositoryResult<bool> {
        repository_boundary(async {
            Self::refresh_pending_slide_classification(self, id, pending).await
        })
        .await
    }
    async fn refresh_pending_slide_embedding(
        &self,
        id: &str,
        subject_document: &str,
        subject_fingerprint: &str,
        capability_document: &str,
        capability_fingerprint: &str,
    ) -> RepositoryResult<bool> {
        repository_boundary(async {
            Self::refresh_pending_slide_embedding(
                self,
                id,
                subject_document,
                subject_fingerprint,
                capability_document,
                capability_fingerprint,
            )
            .await
        })
        .await
    }
    async fn list_retrieval_classifications(
        &self,
        app_id: &str,
    ) -> RepositoryResult<Vec<RetrievalClassificationRecord>> {
        repository_boundary(async { Self::list_retrieval_classifications(self, app_id).await })
            .await
    }
    async fn search_slide_classifications(
        &self,
        app_id: &str,
        fts_query: &str,
        limit: usize,
    ) -> RepositoryResult<Vec<String>> {
        repository_boundary(async {
            Self::search_slide_classifications(self, app_id, fts_query, limit).await
        })
        .await
    }
    async fn ensure_app(
        &self,
        app_id: &str,
        registration: Option<&AppRegistration>,
    ) -> RepositoryResult<StoredApp> {
        repository_boundary(async { Self::ensure_app(self, app_id, registration).await }).await
    }
    async fn find_app(&self, app_id: &str) -> RepositoryResult<Option<StoredApp>> {
        repository_boundary(async { Self::find_app(self, app_id).await }).await
    }

    fn for_user(&self, user_id: i64) -> RepositoryResult<Arc<dyn TemplateRepository>> {
        Ok(Arc::new(self.user_scope(user_id)?))
    }
    async fn for_app(&self, app_id: &str) -> RepositoryResult<Option<Arc<dyn TemplateRepository>>> {
        repository_boundary(async {
            self.ensure_app(app_id, None).await?;
            Ok(Some(Arc::new(Self {
                client: self.client.clone(),
                app_id: app_id.into(),
                owner_user_id: self.owner_user_id,
            }) as Arc<dyn TemplateRepository>))
        })
        .await
    }
    async fn insert(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            Ok(self
                .insert_many(&[TemplateInsert {
                    template: template.clone(),
                    assets: assets.to_vec(),
                    metadata: metadata.clone(),
                    preview: preview.cloned(),
                }])
                .await?)
        })
        .await
    }
    async fn insert_many(&self, records: &[TemplateInsert]) -> RepositoryResult<()> {
        repository_boundary(async { Ok(self.insert_many_cancellable(records, None).await?) }).await
    }
    async fn insert_many_cancellable(
        &self,
        records: &[TemplateInsert],
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            for record in records {
                validate_records(
                    &record.template,
                    &record.assets,
                    &record.metadata,
                    record.preview.as_ref(),
                )?;
            }
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    check_publication(cancellation)?;
                    for record in records {
                        self.write_new(
                            tx,
                            &record.template,
                            &record.assets,
                            &record.metadata,
                            record.preview.as_ref(),
                        )
                        .await?;
                        membership(tx, &self.app_id, &record.template.template_id).await?;
                    }
                    check_publication(cancellation)
                })
                .await
        })
        .await
    }
    async fn insert_with_pending_classification_cancellable(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
        cancellation: Option<&super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            self.insert_classification(record, pending, false, cancellation)
                .await
        })
        .await
    }

    async fn update(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            ensure!(
                assets.iter().all(|a| a.template_id == template.template_id),
                "asset ownership mismatch"
            );
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    ensure!(
                        self.visible(tx, &template.template_id).await?,
                        "template is unavailable to this app"
                    );
                    let builtin = tx
                        .read_one_with(
                            SqlBuilder::select("template_metadata")
                                .columns(["source"])
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .build(),
                            |r| r.get::<_, String>(0),
                        )
                        .await?
                        .is_some_and(|s| s == "builtin");
                    let json = serde_json::to_string(&if builtin {
                        normalized_document(&template.template_json)
                    } else {
                        compact_document(&template.template_json)
                    })?;
                    let changed = tx
                        .write(
                            SqlBuilder::update("templates")
                                .set("template_json", json)
                                .set(
                                    "revision",
                                    crate::shared::revision::next(template.revision)?,
                                )
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .and_where(Condition::equal("revision", template.revision))
                                .build(),
                        )
                        .await?;
                    if changed.rows_affected != 1 {
                        return Err(crate::shared::revision::conflict().into());
                    }
                    insert_assets(tx, assets).await?;
                    Ok(())
                })
                .await
        })
        .await
    }
    async fn upsert_builtin(
        &self,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> RepositoryResult<()> {
        repository_boundary(async {
            validate_records(template, assets, metadata, preview)?;
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let initial_json =
                        serde_json::to_string(&normalized_document(&template.template_json))?;
                    tx.write(
                        SqlBuilder::insert_into("templates")
                            .value("template_id", &template.template_id)
                            .value("template_json", initial_json)
                            .value("owner_user_id", self.owner_user_id)
                            .on_conflict_do_nothing(["template_id"])
                            .build(),
                    )
                    .await?;
                    let current = tx
                        .lock_with(
                            SqlBuilder::select("templates")
                                .columns(["revision"])
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .lock(RowLock::Update, LockWait::Wait)
                                .build(),
                            |r| r.get::<_, i64>(0),
                        )
                        .await?
                        .pop()
                        .context("missing builtin template")?;
                    let existing = tx
                        .read_one_with(
                            SqlBuilder::select("template_metadata")
                                .columns(["source", "checksum"])
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
                        )
                        .await?;
                    let refreshing = existing.is_some();
                    if existing
                        .is_some_and(|(s, c)| s == "import" || c == metadata.catalog_checksum)
                    {
                        return Ok(());
                    }
                    let json =
                        serde_json::to_string(&normalized_document(&template.template_json))?;
                    tx.write(
                        SqlBuilder::insert_into("templates")
                            .value("template_id", &template.template_id)
                            .value("template_json", json)
                            .value("owner_user_id", self.owner_user_id)
                            .on_conflict_update(
                                ConflictUpdate::new(["template_id"]).set_excluded("template_json"),
                            )
                            .build(),
                    )
                    .await?;
                    if refreshing {
                        tx.write(
                            SqlBuilder::update("templates")
                                .set("revision", crate::shared::revision::next(current)?)
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .and_where(Condition::equal("revision", current))
                                .build(),
                        )
                        .await?;
                    }
                    for table in ["template_assets", "template_previews"] {
                        tx.write(
                            SqlBuilder::delete_from(table)
                                .and_where(Condition::equal("template_id", &template.template_id))
                                .build(),
                        )
                        .await?;
                    }
                    write_metadata(tx, metadata, true).await?;
                    insert_assets(tx, assets).await?;
                    write_preview(tx, preview).await?;
                    let apps = tx
                        .read_with(
                            SqlBuilder::select("apps").columns(["app_id"]).build(),
                            |r| r.get::<_, String>(0),
                        )
                        .await?;
                    for app in apps {
                        membership(tx, &app, &template.template_id).await?;
                    }
                    Ok(())
                })
                .await
        })
        .await
    }
    async fn find_by_id(&self, id: &str) -> RepositoryResult<Option<StoredTemplate>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    let json = tx
                        .read_one_with(
                            SqlBuilder::select("templates")
                                .columns(["template_json", "revision"])
                                .and_where(Condition::equal("template_id", id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>("revision")?)),
                        )
                        .await?;
                    json.map(|(json, revision)| {
                        Ok(StoredTemplate {
                            revision,
                            template_id: id.into(),
                            template_json: decode_document(&json)?,
                        })
                    })
                    .transpose()
                })
                .await
        })
        .await
    }
    async fn find_metadata(&self, id: &str) -> RepositoryResult<Option<StoredTemplateMetadata>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    metadata_in(tx, id).await
                })
                .await
        })
        .await
    }
    async fn find_preview(&self, id: &str) -> RepositoryResult<Option<StoredTemplatePreview>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    Ok(tx
                        .read_one_with(
                            SqlBuilder::select("template_previews")
                                .columns(["template_previews.*", "templates.revision"])
                                .inner_join(
                                    "templates",
                                    "templates.template_id",
                                    "template_previews.template_id",
                                )
                                .and_where(Condition::equal("template_previews.template_id", id))
                                .build(),
                            map_preview,
                        )
                        .await?)
                })
                .await
        })
        .await
    }
    async fn list_previews(
        &self,
        limit: usize,
        offset: usize,
    ) -> RepositoryResult<StoredTemplatePreviewPage> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let query = SqlBuilder::select("template_previews")
                        .columns(["template_previews.*", "templates.revision"])
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
                        .and_where(Condition::equal("app_templates.app_id", &self.app_id))
                        .and_where(self.owner_condition());
                    let total = usize::try_from(
                        tx.read_one_with(query.clone().count_rows().build(), |r| {
                            r.get::<_, i64>(0)
                        })
                        .await?
                        .unwrap_or(0),
                    )?;
                    let previews = tx
                        .read_with(
                            query
                                .order_by("template_metadata.created_at", SortDirection::Descending)
                                .order_by("templates.catalog_sequence", SortDirection::Descending)
                                .limit(limit)
                                .offset(offset)
                                .build(),
                            map_preview,
                        )
                        .await?;
                    Ok(StoredTemplatePreviewPage { previews, total })
                })
                .await
        })
        .await
    }
    async fn list(
        &self,
        kind: Option<TemplateKind>,
    ) -> RepositoryResult<Vec<StoredTemplateCatalogItem>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let mut query = SqlBuilder::select("templates")
                        .columns([
                            "templates.template_id",
                            "templates.template_json",
                            "templates.revision",
                        ])
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
                        .and_where(self.owner_condition())
                        .order_by("template_metadata.created_at", SortDirection::Descending)
                        .order_by("templates.catalog_sequence", SortDirection::Descending);
                    if let Some(kind) = kind {
                        query = query
                            .and_where(Condition::equal("template_metadata.kind", kind.as_str()));
                    }
                    let rows = tx
                        .read_with(query.build(), |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, i64>("revision")?,
                            ))
                        })
                        .await?;
                    let mut items = Vec::with_capacity(rows.len());
                    for (id, json, revision) in rows {
                        let metadata = metadata_in(tx, &id)
                            .await?
                            .context("missing template metadata")?;
                        let has_preview = tx
                            .read_one(
                                SqlBuilder::select("template_previews")
                                    .columns(["template_id"])
                                    .and_where(Condition::equal("template_id", &id))
                                    .build(),
                            )
                            .await?
                            .is_some();
                        items.push(StoredTemplateCatalogItem {
                            template: StoredTemplate {
                                revision,
                                template_id: id,
                                template_json: decode_document(&json)?,
                            },
                            metadata,
                            has_preview,
                        })
                    }
                    Ok(items)
                })
                .await
        })
        .await
    }
    async fn delete(&self, id: &str, expected_revision: i64) -> RepositoryResult<bool> {
        repository_boundary(async {
            self.client
                .transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let revisions = tx
                        .lock_with(
                            SqlBuilder::select("templates")
                                .columns(["revision"])
                                .and_where(Condition::equal("template_id", id))
                                .lock(RowLock::Update, LockWait::Wait)
                                .build(),
                            |r| r.get::<_, i64>(0),
                        )
                        .await?;
                    if !self.visible(tx, id).await? {
                        return Ok(false);
                    }
                    if revisions.first().copied() != Some(expected_revision) {
                        return Err(crate::shared::revision::conflict().into());
                    }
                    let changed = tx
                        .write(
                            SqlBuilder::delete_from("app_templates")
                                .and_where(Condition::equal("app_id", &self.app_id))
                                .and_where(Condition::equal("template_id", id))
                                .build(),
                        )
                        .await?
                        .rows_affected;
                    // Keep the canonical built-in/checksum when its last membership is removed.
                    // Reseeding can then distinguish a deletion choice from an unseeded template.
                    let builtin = metadata_in(tx, id)
                        .await?
                        .is_some_and(|metadata| metadata.source == TemplateSource::Builtin);
                    if !builtin
                        && tx
                            .read_one(
                                SqlBuilder::select("app_templates")
                                    .and_where(Condition::equal("template_id", id))
                                    .limit(1)
                                    .build(),
                            )
                            .await?
                            .is_none()
                    {
                        for table in ["slide_classifications_fts", "templates"] {
                            tx.write(
                                SqlBuilder::delete_from(table)
                                    .and_where(Condition::equal("template_id", id))
                                    .build(),
                            )
                            .await?;
                        }
                    }
                    Ok(changed > 0)
                })
                .await
        })
        .await
    }
    async fn find_by_id_with_assets(
        &self,
        id: &str,
    ) -> RepositoryResult<Option<StoredTemplateWithAssets>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    let (json, revision) = tx
                        .read_one_with(
                            SqlBuilder::select("templates")
                                .columns(["template_json", "revision"])
                                .and_where(Condition::equal("template_id", id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>("revision")?)),
                        )
                        .await?
                        .context("missing template")?;
                    let assets = tx
                        .read_with(
                            SqlBuilder::select("template_assets")
                                .and_where(Condition::equal("template_id", id))
                                .order_by("asset_id", SortDirection::Ascending)
                                .build(),
                            map_asset,
                        )
                        .await?;
                    Ok(Some(StoredTemplateWithAssets {
                        revision,
                        template_id: id.into(),
                        template_json: decode_document(&json)?,
                        assets,
                    }))
                })
                .await
        })
        .await
    }
    async fn find_asset(&self, id: &str, asset: &str) -> RepositoryResult<Option<TemplateAsset>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    if !self.visible(tx, id).await? {
                        return Ok(None);
                    }
                    Ok(tx
                        .read_one_with(
                            SqlBuilder::select("template_assets")
                                .and_where(Condition::equal("template_id", id))
                                .and_where(Condition::equal("asset_id", asset))
                                .build(),
                            map_asset,
                        )
                        .await?)
                })
                .await
        })
        .await
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
impl PostgresTemplateRepository {
    async fn write_new(
        &self,
        tx: &PostgresTransaction,
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
                .value("owner_user_id", self.owner_user_id)
                .build(),
        )
        .await?;
        write_metadata(tx, metadata, false).await?;
        insert_assets(tx, assets).await?;
        write_preview(tx, preview).await
    }
}
async fn membership(tx: &PostgresTransaction, app: &str, id: &str) -> anyhow::Result<()> {
    tx.write(
        SqlBuilder::insert_into("app_templates")
            .value("app_id", app)
            .value("template_id", id)
            .value("created_at", chrono::Utc::now().to_rfc3339())
            .on_conflict_do_nothing(["app_id", "template_id"])
            .build(),
    )
    .await?;
    Ok(())
}
impl PostgresTemplateRepository {
    fn owner_condition(&self) -> Condition {
        match self.owner_user_id {
            Some(owner) => Condition::equal("templates.owner_user_id", owner),
            None => Condition::is_not_null("templates.template_id"),
        }
    }
    async fn visible(&self, tx: &PostgresTransaction, id: &str) -> anyhow::Result<bool> {
        Ok(tx
            .read_one(
                SqlBuilder::select("app_templates")
                    .column("app_templates.template_id")
                    .inner_join(
                        "templates",
                        "templates.template_id",
                        "app_templates.template_id",
                    )
                    .and_where(Condition::equal("app_templates.app_id", &self.app_id))
                    .and_where(Condition::equal("app_templates.template_id", id))
                    .and_where(self.owner_condition())
                    .build(),
            )
            .await?
            .is_some())
    }
}

async fn write_metadata(
    tx: &PostgresTransaction,
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
    tx.write(q.build()).await?;
    Ok(())
}
async fn write_preview(
    tx: &PostgresTransaction,
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
        )
        .await?;
    }
    Ok(())
}
async fn insert_assets(tx: &PostgresTransaction, assets: &[TemplateAsset]) -> anyhow::Result<()> {
    for a in assets {
        tx.write(
            SqlBuilder::insert_into("template_assets")
                .value("asset_id", &a.asset_id)
                .value("template_id", &a.template_id)
                .value("content_type", &a.content_type)
                .value("asset_data", a.bytes.as_slice())
                .build(),
        )
        .await?;
    }
    Ok(())
}
fn map_asset(r: &crate::adapters::postgres::client::Row<'_>) -> RowResult<TemplateAsset> {
    Ok(TemplateAsset {
        asset_id: r.get("asset_id")?,
        template_id: r.get("template_id")?,
        content_type: r.get("content_type")?,
        bytes: r.get("asset_data")?,
    })
}
fn map_preview(r: &crate::adapters::postgres::client::Row<'_>) -> RowResult<StoredTemplatePreview> {
    Ok(StoredTemplatePreview {
        revision: r.get("revision")?,
        template_id: r.get("template_id")?,
        content_type: r.get("content_type")?,
        bytes: r.get("preview_data")?,
        width: r.get("width")?,
        height: r.get("height")?,
    })
}
async fn metadata_in(
    tx: &PostgresTransaction,
    id: &str,
) -> anyhow::Result<Option<StoredTemplateMetadata>> {
    let row = tx
        .read_one_with(
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
        )
        .await?;
    let Some((kind, description, source, created, checksum)) = row else {
        return Ok(None);
    };
    let json = tx
        .read_one_with(
            SqlBuilder::select("templates")
                .columns(["template_json"])
                .and_where(Condition::equal("template_id", id))
                .build(),
            |r| r.get::<_, String>(0),
        )
        .await?
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
    .map(Some)
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

async fn repository_boundary<T>(
    operation: impl std::future::Future<Output = anyhow::Result<T>>,
) -> RepositoryResult<T> {
    operation
        .await
        .map_err(|error| match error.downcast::<RepositoryError>() {
            Ok(error) => error,
            Err(error) => RepositoryError::from(error),
        })
}
