use super::{model::*, repository::MembershipRepository};
use crate::shared::error::ServiceResult;
#[derive(Clone)]
pub struct DealAccessService {
    repository: MembershipRepository,
    lifecycle: crate::domains::deals::deletion::lifecycle::DealLifecycle,
}
impl DealAccessService {
    pub fn new(repository: MembershipRepository) -> Self {
        Self {
            repository,
            lifecycle: Default::default(),
        }
    }
    pub(crate) fn lifecycle(&self) -> &crate::domains::deals::deletion::lifecycle::DealLifecycle {
        &self.lifecycle
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
        match mutation {
            Mutation::AddMember { email, role } => {
                self.repository.add_member(deal, actor, email, role).await?
            }
            Mutation::ChangeRole {
                user_id,
                role,
                revision,
            } => {
                self.repository
                    .change_role(deal, actor, user_id, role, revision)
                    .await?
            }
            Mutation::RemoveMember { user_id } => {
                self.repository.remove_member(deal, actor, user_id).await?
            }
            Mutation::Transfer { user_id, revision } => {
                self.repository
                    .transfer_ownership(deal, actor, user_id, revision)
                    .await?
            }
            Mutation::CreateTeam { name } => self.repository.create_team(deal, actor, name).await?,
            Mutation::RenameTeam {
                team_id,
                name,
                revision,
            } => {
                self.repository
                    .rename_team(deal, actor, team_id, name, revision)
                    .await?
            }
            Mutation::DeleteTeam { team_id } => {
                self.repository.delete_team(deal, actor, team_id).await?
            }
            Mutation::Assign {
                team_id,
                user_id,
                add,
            } => {
                self.repository
                    .set_team_member(deal, actor, team_id, user_id, add)
                    .await?
            }
        }
        Ok(())
    }
}
