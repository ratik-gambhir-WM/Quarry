//! Focused template persistence capabilities backed by the same PostgreSQL repository.
use super::super::{models::*, retrieval::*};
use super::{check_publication, AppRegistration, RepositoryResult, StoredApp, DEFAULT_APP_ID};
use crate::shared::error::RepositoryError;
use std::sync::Arc;

/// Read image assets under the repository owner/app scope.
#[async_trait::async_trait]
pub trait TemplateAssetRepository: Send + Sync {
    /// Finds an asset only when it belongs to the requested template.
    async fn find_asset(
        &self,
        template_id: &str,
        asset_id: &str,
    ) -> RepositoryResult<Option<TemplateAsset>>;
}

/// Read and mutate canonical templates. Publication includes assets, previews and pending jobs atomically.
#[async_trait::async_trait]
pub trait TemplateCatalogRepository: Send + Sync {
    /// Publish every slide and its pending job in the same transaction.
    async fn insert_many_with_pending_classification_cancellable(
        &self,
        records: &[(TemplateInsert, PendingSlideClassification)],
        cancellation: Option<&super::super::RequestCancellation>,
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
        cancellation: Option<&super::super::RequestCancellation>,
    ) -> RepositoryResult<()> {
        check_publication(cancellation)?;
        self.insert_many(records).await
    }
    async fn insert_with_pending_classification_cancellable(
        &self,
        record: &TemplateInsert,
        pending: &PendingSlideClassification,
        cancellation: Option<&super::super::RequestCancellation>,
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
}

/// Claim, checkpoint and complete durable classification and embedding work.
#[async_trait::async_trait]
pub trait TemplateProcessingRepository: Send + Sync {
    /// Whether this scope has unfinished stages, including delayed retries and leases.
    async fn has_pending_slide_processing(&self) -> RepositoryResult<bool>;
    /// Atomically reset only a terminal failed stage in this app's saved template.
    async fn retry_failed_slide_processing(
        &self,
        id: &str,
    ) -> RepositoryResult<RetrySlideProcessingOutcome>;
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
}

/// Read the scoped lexical and classified retrieval projection.
#[async_trait::async_trait]
pub trait TemplateRetrievalRepository: Send + Sync {
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
}

/// Composition boundary for owner/app scoping and consumers that coordinate multiple capabilities.
#[async_trait::async_trait]
pub trait TemplateRepository:
    TemplateAssetRepository
    + TemplateCatalogRepository
    + TemplateProcessingRepository
    + TemplateRetrievalRepository
{
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
}
