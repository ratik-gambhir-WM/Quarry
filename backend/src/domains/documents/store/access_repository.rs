//! Bounded final document-result authorization over canonical SQL state.
use crate::{
    adapters::postgres::{
        client::PostgresClient,
        query::{Condition, SqlBuilder},
    },
    shared::error::RepositoryError,
};
#[derive(Clone)]
pub(crate) struct DocumentAccessRepository {
    postgres: PostgresClient,
}
impl DocumentAccessRepository {
    pub fn new(postgres: PostgresClient) -> Self {
        Self { postgres }
    }
    pub async fn current_result_ids(
        &self,
        actor: i64,
        ids: Vec<String>,
    ) -> Result<std::collections::HashSet<String>, RepositoryError> {
        if ids.is_empty() {
            return Ok(std::collections::HashSet::new());
        }
        if ids.len() > super::chunk::MAX_FILE_CHUNK_SEARCH_LIMIT {
            return Err(RepositoryError::invalid(
                "search result authorization bound exceeded",
            ));
        }
        let rows = self
            .postgres
            .read_with_async(
                SqlBuilder::select("document_chunks")
                    .columns([
                        "document_chunk_identities.chunk_id",
                        "document_chunk_identities.index_generation",
                        "document_identity_manifests.active_generation",
                    ])
                    .inner_join(
                        "document_chunk_identities",
                        "document_chunks.chunk_id",
                        "document_chunk_identities.chunk_id",
                    )
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
                    .and_where(Condition::equal(
                        "document_identity_manifests.search_ready",
                        1,
                    ))
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
            .await?;
        Ok(rows
            .into_iter()
            .filter(|(_, generation, active)| generation == active)
            .map(|(id, _, _)| id)
            .collect())
    }
}
