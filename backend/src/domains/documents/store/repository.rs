//! DocumentStore facade and shared strict row codecs.
use serde::Serialize;

use super::{
    publication_repository::persist_file_blob,
    read_repository::{
        find_current_postgres_file_by_content_hash, get_current_deal_document_blob,
        list_current_deal_documents,
    },
};
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, SqlRow},
        query::SqlValue,
    },
    domains::documents::{
        classification::{model::DocumentClassification, repository::ClassificationRecord},
        store::model::{FilePersistenceInput, PersistedFileIdentity},
    },
    shared::error::RepositoryError,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealDocumentSummary {
    pub added_at: String,
    /// Deal-scoped content address: `sha256(deal_id || 0x00 || content_sha256)`.
    pub file_id: String,
    /// SHA-256 of the stored bytes; compare with a local file's hash to detect change.
    pub content_sha256: String,
    pub display_name: String,
    pub size_bytes: i64,
    pub uploaded_by_user_id: Option<i64>,
    pub uploaded_by_name: Option<String>,
    pub attribution_origin: String,
    /// Optional enrichment for exactly this current version; `not_requested` when absent.
    pub classification: DocumentClassification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocumentBlob {
    pub file_id: String,
    pub display_name: String,
    pub mime_type: String,
    pub file_bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct DocumentStore {
    postgres: PostgresClient,
    profile: super::search_repository::EmbeddingProfile,
}

impl DocumentStore {
    pub fn new(postgres: PostgresClient) -> Self {
        Self {
            postgres,
            profile: super::search_repository::EmbeddingProfile::new(
                "text-embedding-3-small".into(),
                1536,
            ),
        }
    }

    pub(crate) fn with_profile(mut self, model: String, dimensions: usize) -> Self {
        self.profile = super::search_repository::EmbeddingProfile::new(model, dimensions);
        self
    }
    pub(crate) fn profile(&self) -> &super::search_repository::EmbeddingProfile {
        &self.profile
    }
    /// Reconcile exact-content work under current file, membership and attempt locks.
    pub(crate) async fn complete_current_attempt(
        &self,
        attempt: crate::domains::documents::ingestion::repository::Attempt,
    ) -> Result<bool, RepositoryError> {
        super::publication_repository::complete_current_attempt(
            &self.postgres,
            self.profile.id.clone(),
            attempt,
        )
        .await
    }
    pub async fn manifest(
        &self,
        version: String,
    ) -> Result<Option<super::identity::IdentityManifest>, RepositoryError> {
        super::identity_repository::manifest(&self.postgres, version).await
    }
    pub async fn authorize_read(&self, deal: &str, actor: i64) -> Result<(), RepositoryError> {
        let deal = deal.to_string();
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                crate::domains::memberships::require_deal(
                    tx,
                    &deal,
                    actor,
                    crate::domains::memberships::model::Capability::Read,
                )
                .await
                .map(|_| ())
            })
            .await
    }
    pub async fn authorize_upload(&self, deal: &str, actor: i64) -> Result<(), RepositoryError> {
        let deal = deal.to_string();
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                crate::domains::memberships::require_deal(
                    tx,
                    &deal,
                    actor,
                    crate::domains::memberships::model::Capability::Upload,
                )
                .await
                .map(|_| ())
            })
            .await
    }
    pub async fn list_for_deal(
        &self,
        deal_id: &str,
    ) -> Result<Vec<DealDocumentSummary>, RepositoryError> {
        list_current_deal_documents(&self.postgres, deal_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn current_blob(
        &self,
        deal_id: &str,
        file_id: &str,
    ) -> Result<Option<StoredDocumentBlob>, RepositoryError> {
        get_current_deal_document_blob(&self.postgres, deal_id, file_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn find_current_by_hash(
        &self,
        deal_id: &str,
        content_sha256: &str,
    ) -> Result<Option<PersistedFileIdentity>, RepositoryError> {
        find_current_postgres_file_by_content_hash(&self.postgres, deal_id, content_sha256)
            .await
            .map_err(RepositoryError::storage)
    }

    pub(crate) async fn persist_complete(
        &self,
        input: FilePersistenceInput,
        chunks: super::search_repository::PreparedChunks,
        attempt: Option<crate::domains::documents::ingestion::repository::Attempt>,
        classification: Option<ClassificationRecord>,
    ) -> Result<PersistedFileIdentity, RepositoryError> {
        persist_file_blob(&self.postgres, input, chunks, attempt, classification).await
    }
}

pub(super) fn required_text(row: &SqlRow, column: &str) -> Result<String, PostgresError> {
    match row.get(column) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(invalid_row_value(column, "TEXT")),
    }
}

pub(super) fn required_integer(row: &SqlRow, column: &str) -> Result<i64, PostgresError> {
    match row.get(column) {
        Some(SqlValue::Integer(value)) => Ok(*value),
        _ => Err(invalid_row_value(column, "INTEGER")),
    }
}

fn invalid_row_value(column: &str, expected: &str) -> PostgresError {
    PostgresError::transaction_aborted(format!(
        "transaction row column `{column}` was missing or was not {expected}"
    ))
}

#[cfg(test)]
#[path = "../../../../tests/integration/domains/documents/store/repository_tests.rs"]
mod tests;
