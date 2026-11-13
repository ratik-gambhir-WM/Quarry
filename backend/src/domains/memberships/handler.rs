use validate::{member_email, name, non_owner, paging, revision, team, user};
mod validate;
use super::{model::*, service::DealAccessService};
use crate::{
    app::http::error::{AppError, AppResult},
    domains::identity::model::AuthenticatedUser,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Paging {
    cursor: Option<String>,
    limit: Option<usize>,
}
pub(super) async fn members(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path(d): Path<String>,
    Query(p): Query<Paging>,
) -> AppResult<Json<Page<Member>>> {
    let (cursor, limit) = paging(p)?;
    let after = if cursor.is_empty() {
        0
    } else {
        cursor
            .parse::<i64>()
            .ok()
            .filter(|v| *v > 0)
            .ok_or_else(|| AppError::bad_request("invalid cursor"))?
    };
    Ok(Json(s.members(d, a.user.id, after, limit).await?))
}
pub(super) async fn teams(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path(d): Path<String>,
    Query(p): Query<Paging>,
) -> AppResult<Json<Page<Team>>> {
    let (cursor, limit) = paging(p)?;
    if !cursor.is_empty() {
        team(&cursor)?;
    }
    Ok(Json(s.teams(d, a.user.id, cursor, limit).await?))
}
pub(super) async fn activity(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path(d): Path<String>,
    Query(p): Query<Paging>,
) -> AppResult<Json<Page<Activity>>> {
    let (cursor, limit) = paging(p)?;
    Ok(Json(s.activity(d, a.user.id, cursor, limit).await?))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct AddMember {
    email: String,
    role: DealRole,
}
pub(super) async fn add_member(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path(d): Path<String>,
    Json(b): Json<AddMember>,
) -> AppResult<(StatusCode, Json<()>)> {
    let email = member_email(&b.email)?;
    non_owner(b.role)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::AddMember {
            email,
            role: b.role,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(())))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RoleChange {
    role: DealRole,
    expected_revision: i64,
}
pub(super) async fn change_role(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id)): Path<(String, i64)>,
    Json(b): Json<RoleChange>,
) -> AppResult<StatusCode> {
    user(id)?;
    revision(b.expected_revision)?;
    non_owner(b.role)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::ChangeRole {
            user_id: id,
            role: b.role,
            revision: b.expected_revision,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn remove_member(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id)): Path<(String, i64)>,
    Query(b): Query<DeleteRevision>,
) -> AppResult<StatusCode> {
    user(id)?;
    revision(b.expected_revision)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::RemoveMember {
            user_id: id,
            revision: b.expected_revision,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Transfer {
    user_id: i64,
    expected_access_revision: i64,
}
pub(super) async fn transfer(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path(d): Path<String>,
    Json(b): Json<Transfer>,
) -> AppResult<StatusCode> {
    user(b.user_id)?;
    revision(b.expected_access_revision)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::Transfer {
            user_id: b.user_id,
            revision: b.expected_access_revision,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct TeamName {
    name: String,
}
pub(super) async fn create_team(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path(d): Path<String>,
    Json(b): Json<TeamName>,
) -> AppResult<(StatusCode, Json<()>)> {
    s.mutate(
        d,
        a.user.id,
        Mutation::CreateTeam {
            name: name(&b.name)?,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(())))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RenameTeam {
    name: String,
    expected_revision: i64,
}
pub(super) async fn rename_team(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id)): Path<(String, String)>,
    Json(b): Json<RenameTeam>,
) -> AppResult<StatusCode> {
    team(&id)?;
    revision(b.expected_revision)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::RenameTeam {
            team_id: id,
            name: name(&b.name)?,
            revision: b.expected_revision,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn delete_team(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id)): Path<(String, String)>,
    Query(b): Query<DeleteRevision>,
) -> AppResult<StatusCode> {
    team(&id)?;
    revision(b.expected_revision)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::DeleteTeam {
            team_id: id,
            revision: b.expected_revision,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Assignment {
    user_id: i64,
}
pub(super) async fn assign(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id)): Path<(String, String)>,
    Json(b): Json<Assignment>,
) -> AppResult<StatusCode> {
    team(&id)?;
    user(b.user_id)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::Assign {
            team_id: id,
            user_id: b.user_id,
            add: true,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn unassign(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id, u)): Path<(String, String, i64)>,
) -> AppResult<StatusCode> {
    team(&id)?;
    user(u)?;
    s.mutate(
        d,
        a.user.id,
        Mutation::Assign {
            team_id: id,
            user_id: u,
            add: false,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/memberships/route_tests.rs"]
mod tests;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DeleteRevision {
    expected_revision: i64,
}
