//! Authenticated resource policy and bounded canonical document snapshots.
use crate::{
    adapters::postgres::client::PostgresClient,
    domains::documents::DocumentAccessRepository,
    domains::memberships::{model::Capability, service::DealAccessService},
    shared::error::{ServiceError, ServiceResult},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct ResourceAccess {
    documents: DocumentAccessRepository,
    policy: Arc<DealAccessService>,
}

impl ResourceAccess {
    pub fn new(postgres: PostgresClient, policy: Arc<DealAccessService>) -> Self {
        Self {
            documents: DocumentAccessRepository::new(postgres),
            policy,
        }
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
        if ids.len() > crate::domains::documents::store::chunk::MAX_FILE_CHUNK_SEARCH_LIMIT {
            return Err(ServiceError::internal(
                "search result authorization bound exceeded",
            ));
        }
        Ok(self.documents.current_result_ids(actor, ids).await?)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/identity/access_tests.rs"]
mod tests;
