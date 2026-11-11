use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionStatus {
    Pending,
    Running,
    Retrying,
    Blocked,
    Completed,
}
impl DeletionStatus {
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Retrying => "retrying",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionPhase {
    Draining,
    Fencing,
    Sqlite,
    Graph,
    Verifying,
    Completed,
}
impl DeletionPhase {
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::Draining => "draining",
            Self::Fencing => "fencing",
            Self::Sqlite => "sqlite",
            Self::Graph => "graph",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DealDeletion {
    pub deletion_id: String,
    pub deal_id: String,
    pub status: DeletionStatus,
    pub phase: DeletionPhase,
    pub retryable: bool,
    pub status_url: String,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}
