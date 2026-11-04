use super::{model::*, service::DealAccessService};
use crate::{
    app::http::error::{AppError, AppResult},
    domains::identity::model::AuthenticatedUser,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

pub(crate) fn routes(service: Arc<DealAccessService>) -> Router {
    Router::new()
        .route("/deals/{deal_id}/members", get(members).post(add_member))
        .route(
            "/deals/{deal_id}/members/{user_id}",
            patch(change_role).delete(remove_member),
        )
        .route("/deals/{deal_id}/ownership-transfer", post(transfer))
        .route("/deals/{deal_id}/teams", get(teams).post(create_team))
        .route(
            "/deals/{deal_id}/teams/{team_id}",
            patch(rename_team).delete(delete_team),
        )
        .route("/deals/{deal_id}/teams/{team_id}/members", post(assign))
        .route(
            "/deals/{deal_id}/teams/{team_id}/members/{user_id}",
            axum::routing::delete(unassign),
        )
        .route("/deals/{deal_id}/activity", get(activity))
        .with_state(service)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Paging {
    cursor: Option<String>,
    limit: Option<usize>,
}
fn paging(p: Paging) -> AppResult<(String, usize)> {
    let limit = p.limit.unwrap_or(30);
    let cursor = p.cursor.unwrap_or_default();
    if !(1..=100).contains(&limit) || cursor.len() > 128 || cursor.chars().any(char::is_control) {
        return Err(AppError::bad_request("invalid pagination"));
    }
    Ok((cursor, limit))
}
fn user(id: i64) -> AppResult<()> {
    if !(1..=9_007_199_254_740_991).contains(&id) {
        return Err(AppError::bad_request("invalid userId"));
    }
    Ok(())
}
fn revision(value: i64) -> AppResult<()> {
    if !(1..=9_007_199_254_740_991).contains(&value) {
        return Err(AppError::bad_request("invalid revision"));
    }
    Ok(())
}
fn team(id: &str) -> AppResult<()> {
    uuid::Uuid::parse_str(id).map_err(|_| AppError::bad_request("invalid teamId"))?;
    Ok(())
}
fn name(value: &str) -> AppResult<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty()
        || normalized.chars().count() > 100
        || value.chars().any(char::is_control)
    {
        return Err(AppError::bad_request(
            "team name must contain 1 to 100 characters",
        ));
    }
    Ok(normalized)
}
fn non_owner(role: DealRole) -> AppResult<()> {
    if role == DealRole::Owner {
        return Err(AppError::bad_request("use ownership transfer"));
    }
    Ok(())
}
async fn members(
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
async fn teams(
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
async fn activity(
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
struct AddMember {
    email: String,
    role: DealRole,
}
fn member_email(value: &str) -> AppResult<String> {
    let value = value.trim().to_ascii_lowercase();
    let (local, domain) = value
        .split_once('@')
        .ok_or_else(|| AppError::bad_request("invalid email"))?;
    if value.chars().count() > 320
        || local.is_empty()
        || !domain.contains('.')
        || domain.contains('@')
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AppError::bad_request("invalid email"));
    }
    Ok(value)
}
async fn add_member(
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
struct RoleChange {
    role: DealRole,
    expected_revision: i64,
}
async fn change_role(
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
async fn remove_member(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id)): Path<(String, i64)>,
) -> AppResult<StatusCode> {
    user(id)?;
    s.mutate(d, a.user.id, Mutation::RemoveMember { user_id: id })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Transfer {
    user_id: i64,
    expected_access_revision: i64,
}
async fn transfer(
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
struct TeamName {
    name: String,
}
async fn create_team(
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
struct RenameTeam {
    name: String,
    expected_revision: i64,
}
async fn rename_team(
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
async fn delete_team(
    Extension(a): Extension<AuthenticatedUser>,
    State(s): State<Arc<DealAccessService>>,
    Path((d, id)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    team(&id)?;
    s.mutate(d, a.user.id, Mutation::DeleteTeam { team_id: id })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Assignment {
    user_id: i64,
}
async fn assign(
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
async fn unassign(
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
