pub(crate) mod extraction;
pub(crate) mod extraction_handler;
pub(crate) mod handler;
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
