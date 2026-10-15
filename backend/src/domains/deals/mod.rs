pub(crate) mod extraction_handler;
pub(crate) mod handler;
pub(crate) mod model;
pub(crate) mod repository;
pub(crate) mod route;
pub(crate) mod service;
pub(crate) mod upload;

use repository::DealRepository;

#[derive(Clone)]
pub struct DataRoomSourceReader {
    repository: DealRepository,
}

impl DataRoomSourceReader {
    pub(crate) fn new(repository: DealRepository) -> Self {
        Self { repository }
    }

    pub async fn local_root(
        &self,
        deal_id: String,
    ) -> Result<Option<String>, crate::shared::error::ServiceError> {
        Ok(self
            .repository
            .metadata(deal_id)
            .await?
            .and_then(|metadata| metadata.local_path))
    }
}

/// Bounded, current deal reference data for other domains. It is not document retrieval.
#[derive(Clone)]
pub struct DealContextReader {
    repository: DealRepository,
}
impl DealContextReader {
    pub(crate) fn new(repository: DealRepository) -> Self {
        Self { repository }
    }
    pub async fn read(&self, id: &str, owner: i64) -> crate::shared::error::ServiceResult<String> {
        use crate::shared::error::ServiceError;
        let deal = self
            .repository
            .by_id(id.to_string())
            .await?
            .ok_or_else(|| ServiceError::not_found("deal not found"))?;
        if deal.user_id != owner {
            return Err(ServiceError::Conflict(
                "deal and thread must have the same owner".into(),
            ));
        }
        // Serialize reference values so names cannot escape their data fields. Never include source paths or documents.
        let parties = deal.parties.iter().take(100).map(|p| serde_json::json!({"role": p.role, "name": p.company.name.chars().take(300).collect::<String>()})).collect::<Vec<_>>();
        Ok(serde_json::json!({"dealName": deal.deal_name.chars().take(300).collect::<String>(), "engagementType": deal.engagement_type, "transactionType": deal.transaction_type, "parties": parties, "scopes": deal.scopes}).to_string())
    }
}
