use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DealRole {
    Owner,
    Manager,
    Contributor,
    Viewer,
}
impl DealRole {
    pub fn code(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Manager => "manager",
            Self::Contributor => "contributor",
            Self::Viewer => "viewer",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Read,
    Upload,
    Edit,
    ManageTeams,
    ManageMembers,
    Transfer,
    Archive,
    LocalSource,
    RemoveMember,
    Leave,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealCapabilities {
    pub read: bool,
    pub upload: bool,
    pub edit: bool,
    pub manage_teams: bool,
    pub manage_members: bool,
    pub transfer: bool,
    pub archive: bool,
    pub local_source: bool,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealAccess {
    pub role: DealRole,
    pub access_revision: i64,
    pub created_by_user_id: i64,
    pub archived: bool,
    pub capabilities: DealCapabilities,
}
impl DealAccess {
    pub(crate) fn for_actor(
        role: DealRole,
        creator: i64,
        archived: bool,
        revision: i64,
        actor: i64,
    ) -> Self {
        let owner = role == DealRole::Owner;
        let manager = owner || role == DealRole::Manager;
        Self {
            role,
            created_by_user_id: creator,
            archived,
            access_revision: revision,
            capabilities: DealCapabilities {
                read: true,
                upload: !archived && role != DealRole::Viewer,
                edit: !archived && manager,
                manage_teams: !archived && manager,
                manage_members: !archived && owner,
                transfer: owner,
                archive: !archived && owner,
                local_source: owner && actor == creator,
            },
        }
    }

    pub fn permits(&self, capability: Capability) -> bool {
        match capability {
            Capability::Read => true,
            Capability::Upload => self.capabilities.upload,
            Capability::Edit => self.capabilities.edit,
            Capability::ManageTeams => self.capabilities.manage_teams,
            Capability::ManageMembers => self.capabilities.manage_members,
            Capability::Transfer => self.capabilities.transfer,
            Capability::Archive => self.capabilities.archive,
            Capability::LocalSource => self.capabilities.local_source,
            Capability::RemoveMember => self.role == DealRole::Owner,
            Capability::Leave => self.role != DealRole::Owner,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub user_id: i64,
    pub username: String,
    pub first_name: String,
    pub last_name: String,
    pub role: DealRole,
    pub revision: i64,
    pub joined_at: String,
    pub team_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub team_id: String,
    pub name: String,
    pub revision: i64,
    pub user_ids: Vec<i64>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub event_id: String,
    pub actor_user_id: Option<i64>,
    pub actor_name: Option<String>,
    pub kind: String,
    pub target_id: Option<String>,
    pub created_at: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone)]
pub enum Mutation {
    AddMember {
        username: String,
        role: DealRole,
    },
    ChangeRole {
        user_id: i64,
        role: DealRole,
        revision: i64,
    },
    RemoveMember {
        user_id: i64,
    },
    Transfer {
        user_id: i64,
        revision: i64,
    },
    CreateTeam {
        name: String,
    },
    RenameTeam {
        team_id: String,
        name: String,
        revision: i64,
    },
    DeleteTeam {
        team_id: String,
    },
    Assign {
        team_id: String,
        user_id: i64,
        add: bool,
    },
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/memberships/model_tests.rs"]
mod tests;
