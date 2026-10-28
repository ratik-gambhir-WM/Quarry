//! Canonical resource ownership reads; authenticated IDs are supplied by the HTTP boundary.
use crate::{
    adapters::sqlite::{
        client::SqliteClient,
        query::{Condition, SqlBuilder},
    },
    shared::error::{ServiceError, ServiceResult},
};
#[derive(Clone)]
pub struct ResourceAccess {
    sqlite: SqliteClient,
}
impl ResourceAccess {
    pub fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }
    pub async fn deal(&self, id: String, owner: i64) -> ServiceResult<()> {
        let found = self
            .sqlite
            .read_one_with_async(
                SqlBuilder::select("deals")
                    .columns(["deal_id"])
                    .and_where(Condition::equal("deal_id", id))
                    .and_where(Condition::equal("user_id", owner))
                    .build(),
                |_| Ok(()),
            )
            .await
            .map_err(|_| ServiceError::internal("ownership lookup failed"))?;
        found.ok_or_else(|| ServiceError::not_found("resource not found"))
    }
    pub async fn live_chunk(
        &self,
        owner: i64,
        file: String,
        version: String,
    ) -> ServiceResult<bool> {
        let owned_deal = self
            .sqlite
            .read_one_with_async(
                SqlBuilder::select("quarry_files")
                    .columns(["deal_id"])
                    .and_where(Condition::equal("file_id", file.clone()))
                    .and_where(Condition::is_null("deleted_at"))
                    .build(),
                |r| r.get::<_, String>(0),
            )
            .await
            .map_err(|_| ServiceError::internal("file ownership lookup failed"))?;
        let Some(deal) = owned_deal else {
            return Ok(false);
        };
        match self.deal(deal, owner).await {
            Ok(()) => {}
            Err(ServiceError::NotFound(_)) => return Ok(false),
            Err(error) => return Err(error),
        }
        Ok(self
            .sqlite
            .read_one_with_async(
                SqlBuilder::select("quarry_file_versions")
                    .columns(["version_id"])
                    .and_where(Condition::equal("version_id", version))
                    .and_where(Condition::equal("file_id", file))
                    .and_where(Condition::equal("is_current", true))
                    .build(),
                |_| Ok(()),
            )
            .await
            .map_err(|_| ServiceError::internal("version lookup failed"))?
            .is_some())
    }
}
