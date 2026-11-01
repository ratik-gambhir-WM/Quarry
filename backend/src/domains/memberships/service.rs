use super::{model::*, repository::MembershipRepository};
use crate::shared::error::ServiceResult;
#[derive(Clone)]
pub struct DealAccessService {
    repository: MembershipRepository,
}
impl DealAccessService {
    pub fn new(repository: MembershipRepository) -> Self {
        Self { repository }
    }
    pub async fn require(
        &self,
        deal: &str,
        actor: i64,
        capability: Capability,
    ) -> ServiceResult<DealAccess> {
        Ok(self
            .repository
            .access(deal.into(), actor, capability)
            .await?)
    }
    pub async fn accessible_deals(&self, actor: i64) -> ServiceResult<Vec<String>> {
        Ok(self.repository.accessible_deals(actor).await?)
    }
    pub async fn members(
        &self,
        deal: String,
        actor: i64,
        after: i64,
        limit: usize,
    ) -> ServiceResult<Page<Member>> {
        Ok(self.repository.members(deal, actor, after, limit).await?)
    }
    pub async fn teams(
        &self,
        deal: String,
        actor: i64,
        after: String,
        limit: usize,
    ) -> ServiceResult<Page<Team>> {
        Ok(self.repository.teams(deal, actor, after, limit).await?)
    }
    pub async fn activity(
        &self,
        deal: String,
        actor: i64,
        after: String,
        limit: usize,
    ) -> ServiceResult<Page<Activity>> {
        Ok(self.repository.activity(deal, actor, after, limit).await?)
    }
    pub async fn mutate(&self, deal: String, actor: i64, mutation: Mutation) -> ServiceResult<()> {
        Ok(self.repository.mutate(deal, actor, mutation).await?)
    }
}
