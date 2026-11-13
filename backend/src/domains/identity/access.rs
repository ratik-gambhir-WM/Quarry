//! Authenticated resource policy and bounded canonical document snapshots.
use crate::{
    adapters::postgres::{
        client::PostgresClient,
        query::{Condition, SqlBuilder},
    },
    domains::{
        documents::store::identity::ChunkIdentity,
        memberships::{model::Capability, service::DealAccessService},
    },
    shared::error::{RepositoryError, ServiceError, ServiceResult},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct ResourceAccess {
    postgres: PostgresClient,
    policy: Arc<DealAccessService>,
}

#[derive(Debug, Clone)]
pub struct CanonicalChunk {
    pub file_id: String,
    pub version_id: String,
    pub identity: ChunkIdentity,
}

impl ResourceAccess {
    pub fn new(postgres: PostgresClient, policy: Arc<DealAccessService>) -> Self {
        Self { postgres, policy }
    }

    pub async fn deal(&self, id: String, actor: i64) -> ServiceResult<()> {
        self.policy
            .require(&id, actor, Capability::Read)
            .await
            .map(|_| ())
    }

    pub fn policy(&self) -> &DealAccessService {
        &self.policy
    }

    /// Final bounded result check, in one snapshot across all returned deals.
    pub async fn current_result_ids(
        &self,
        actor: i64,
        ids: Vec<String>,
    ) -> ServiceResult<std::collections::HashSet<String>> {
        if ids.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        if ids.len() > crate::domains::documents::graph::query::MAX_FILE_CHUNK_SEARCH_LIMIT {
            return Err(ServiceError::internal(
                "search result authorization bound exceeded",
            ));
        }
        let rows = self
            .postgres
            .read_with_async(
                SqlBuilder::select("document_chunk_identities")
                    .columns([
                        "document_chunk_identities.chunk_id",
                        "document_chunk_identities.index_generation",
                        "document_identity_manifests.active_generation",
                    ])
                    .inner_join(
                        "document_identity_manifests",
                        "document_chunk_identities.version_id",
                        "document_identity_manifests.version_id",
                    )
                    .inner_join(
                        "quarry_file_versions",
                        "document_chunk_identities.version_id",
                        "quarry_file_versions.version_id",
                    )
                    .inner_join(
                        "quarry_files",
                        "quarry_file_versions.file_id",
                        "quarry_files.file_id",
                    )
                    .inner_join(
                        "deal_memberships",
                        "quarry_files.deal_id",
                        "deal_memberships.deal_id",
                    )
                    .left_join(
                        "deal_deletions",
                        "quarry_files.deal_id",
                        "deal_deletions.deal_id",
                    )
                    .and_where(Condition::is_null("deal_deletions.deal_id"))
                    .and_where(Condition::equal("deal_memberships.user_id", actor))
                    .and_where(Condition::equal("quarry_file_versions.is_current", true))
                    .and_where(Condition::is_null("quarry_files.deleted_at"))
                    .and_where(Condition::in_values(
                        "document_chunk_identities.chunk_id",
                        ids,
                    ))
                    .build(),
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .await
            .map_err(RepositoryError::from)?;
        Ok(rows
            .into_iter()
            .filter(|(_, generation, active)| generation == active)
            .map(|(id, _, _)| id)
            .collect())
    }

    /// One canonical snapshot, before ranking and again before exposing any text.
    pub async fn current_chunks(
        &self,
        actor: i64,
        deal: &str,
    ) -> ServiceResult<Vec<CanonicalChunk>> {
        let deal = deal.to_string();
        let result = self
            .postgres
            .read_transaction_result_async::<_, RepositoryError, _, _>(move |tx| async move {
                let tx = &tx;
                crate::domains::memberships::require_deal(tx, &deal, actor, Capability::Read)
                    .await?;
                let versions = tx
                    .read_with(
                        SqlBuilder::select("quarry_files")
                            .columns([
                                "quarry_file_versions.version_id",
                                "document_identity_manifests.chunk_count",
                            ])
                            .inner_join(
                                "quarry_file_versions",
                                "quarry_files.file_id",
                                "quarry_file_versions.file_id",
                            )
                            .left_join(
                                "document_identity_manifests",
                                "quarry_file_versions.version_id",
                                "document_identity_manifests.version_id",
                            )
                            .and_where(Condition::equal("quarry_files.deal_id", deal))
                            .and_where(Condition::is_null("quarry_files.deleted_at"))
                            .and_where(Condition::equal("quarry_file_versions.is_current", true))
                            .limit(5001)
                            .build(),
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
                    )
                    .await?;
                if versions.len() > 5000 {
                    return Err(RepositoryError::Unavailable(
                        "deal search exceeds 5,000 current documents".into(),
                    ));
                }
                if versions.is_empty() {
                    return Ok(Vec::new());
                }
                let mut expected = 0_i64;
                for (_, count) in &versions {
                    expected = expected
                        .checked_add(count.ok_or_else(|| {
                            RepositoryError::Unavailable(
                                "document identity migration is incomplete".into(),
                            )
                        })?)
                        .ok_or_else(|| RepositoryError::invalid("chunk count overflow"))?;
                }
                if expected > 10000 {
                    return Err(RepositoryError::Unavailable(
                        "deal search exceeds 10,000 canonical chunks".into(),
                    ));
                }
                let rows = tx
                    .read_with(
                        SqlBuilder::select("document_chunk_identities")
                            .columns([
                                "quarry_file_versions.file_id",
                                "document_chunk_identities.version_id",
                                "document_chunk_identities.chunk_id",
                                "document_chunk_identities.chunk_index",
                                "document_chunk_identities.index_generation",
                                "document_chunk_identities.chunk_sha256",
                                "document_identity_manifests.active_generation",
                            ])
                            .inner_join(
                                "quarry_file_versions",
                                "document_chunk_identities.version_id",
                                "quarry_file_versions.version_id",
                            )
                            .inner_join(
                                "document_identity_manifests",
                                "document_chunk_identities.version_id",
                                "document_identity_manifests.version_id",
                            )
                            .and_where(Condition::in_values(
                                "document_chunk_identities.version_id",
                                versions.iter().map(|(id, _)| id.as_str()),
                            ))
                            .limit(10001)
                            .build(),
                        |row| {
                            Ok((
                                CanonicalChunk {
                                    file_id: row.get("file_id")?,
                                    version_id: row.get("version_id")?,
                                    identity: ChunkIdentity {
                                        chunk_id: row.get("chunk_id")?,
                                        index_generation: row.get("index_generation")?,
                                        chunk_index: row.get("chunk_index")?,
                                        chunk_sha256: row.get("chunk_sha256")?,
                                    },
                                },
                                row.get::<_, String>("active_generation")?,
                            ))
                        },
                    )
                    .await?;
                if i64::try_from(rows.len()).ok() != Some(expected) {
                    return Err(RepositoryError::Unavailable(
                        "document identity manifest is incomplete".into(),
                    ));
                }
                Ok(rows
                    .into_iter()
                    .filter(|(chunk, active)| chunk.identity.index_generation == *active)
                    .map(|(chunk, _)| chunk)
                    .collect())
            })
            .await;
        result.map_err(ServiceError::from)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/identity/access_tests.rs"]
mod tests;
