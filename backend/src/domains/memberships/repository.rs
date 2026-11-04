//! Canonical deal policy and atomic roster/activity mutations.
use super::model::{Activity, Capability, DealAccess, DealRole, Member, Page, Team};
use crate::{
    adapters::sqlite::{
        client::{SqliteClient, SqliteClientError, SqliteTransaction},
        query::{ComparisonOperator, Condition, SortDirection, SqlBuilder},
    },
    shared::error::RepositoryError,
};

#[derive(Clone)]
pub struct MembershipRepository {
    sqlite: SqliteClient,
}
impl MembershipRepository {
    pub fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }
    pub async fn access(
        &self,
        deal: String,
        actor: i64,
        capability: Capability,
    ) -> Result<DealAccess, RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| require(tx, &deal, actor, capability))
            .await
    }
    pub async fn accessible_deals(&self, actor: i64) -> Result<Vec<String>, RepositoryError> {
        Ok(self
            .sqlite
            .read_with_async(
                SqlBuilder::select("deal_memberships")
                    .columns(["deal_id"])
                    .and_where(Condition::equal("user_id", actor))
                    .order_by("deal_id", SortDirection::Ascending)
                    // Global search rejects the sentinel row rather than returning a partial set.
                    .limit(201)
                    .build(),
                |r| r.get(0),
            )
            .await?)
    }
    pub async fn members(
        &self,
        deal: String,
        actor: i64,
        after: i64,
        limit: usize,
    ) -> Result<Page<Member>, RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                require(tx, &deal, actor, Capability::Read)?;
                let mut items = tx.read_with(
                    SqlBuilder::select("deal_memberships")
                        .columns([
                            "users.id",
                            "users.username",
                            "users.first_name",
                            "users.last_name",
                            "deal_memberships.role",
                            "deal_memberships.revision",
                            "deal_memberships.joined_at",
                        ])
                        .inner_join("users", "deal_memberships.user_id", "users.id")
                        .and_where(Condition::equal("deal_id", &deal))
                        .and_where(Condition::compare(
                            "users.id",
                            ComparisonOperator::GreaterThan,
                            after,
                        ))
                        .order_by("users.id", SortDirection::Ascending)
                        .limit(limit + 1)
                        .build(),
                    |r| {
                        Ok(Member {
                            user_id: r.get("id")?,
                            username: r.get("username")?,
                            first_name: r.get("first_name")?,
                            last_name: r.get("last_name")?,
                            role: role_from_row(r, "role")?,
                            revision: r.get("revision")?,
                            joined_at: r.get("joined_at")?,
                            team_ids: Vec::new(),
                        })
                    },
                )?;
                let next_cursor = if items.len() > limit {
                    items.truncate(limit);
                    items.last().map(|m| m.user_id.to_string())
                } else {
                    None
                };
                if !items.is_empty() {
                    let assignments = tx.read_with(
                        SqlBuilder::select("deal_team_memberships")
                            .and_where(Condition::equal("deal_id", &deal))
                            .and_where(Condition::in_values(
                                "user_id",
                                items.iter().map(|m| m.user_id),
                            ))
                            .order_by("team_id", SortDirection::Ascending)
                            .build(),
                        |r| Ok((r.get::<_, i64>("user_id")?, r.get::<_, String>("team_id")?)),
                    )?;
                    for member in &mut items {
                        member.team_ids = assignments
                            .iter()
                            .filter(|(id, _)| *id == member.user_id)
                            .map(|(_, id)| id.clone())
                            .collect();
                    }
                }
                Ok(Page { items, next_cursor })
            })
            .await
    }
    pub async fn teams(
        &self,
        deal: String,
        actor: i64,
        after: String,
        limit: usize,
    ) -> Result<Page<Team>, RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                require(tx, &deal, actor, Capability::Read)?;
                let mut items = tx.read_with(
                    SqlBuilder::select("deal_teams")
                        .and_where(Condition::equal("deal_id", &deal))
                        .and_where(Condition::compare(
                            "team_id",
                            ComparisonOperator::GreaterThan,
                            after,
                        ))
                        .order_by("team_id", SortDirection::Ascending)
                        .limit(limit + 1)
                        .build(),
                    |r| {
                        Ok(Team {
                            team_id: r.get("team_id")?,
                            name: r.get("name")?,
                            revision: r.get("revision")?,
                            user_ids: Vec::new(),
                        })
                    },
                )?;
                let next_cursor = if items.len() > limit {
                    items.truncate(limit);
                    items.last().map(|m| m.team_id.clone())
                } else {
                    None
                };
                if !items.is_empty() {
                    let assignments = tx.read_with(
                        SqlBuilder::select("deal_team_memberships")
                            .and_where(Condition::equal("deal_id", &deal))
                            .and_where(Condition::in_values(
                                "team_id",
                                items.iter().map(|t| t.team_id.as_str()),
                            ))
                            .order_by("user_id", SortDirection::Ascending)
                            .build(),
                        |r| Ok((r.get::<_, String>("team_id")?, r.get::<_, i64>("user_id")?)),
                    )?;
                    for team in &mut items {
                        team.user_ids = assignments
                            .iter()
                            .filter(|(id, _)| *id == team.team_id)
                            .map(|(_, id)| *id)
                            .collect();
                    }
                }
                Ok(Page { items, next_cursor })
            })
            .await
    }
    pub async fn activity(
        &self,
        deal: String,
        actor: i64,
        after: String,
        limit: usize,
    ) -> Result<Page<Activity>, RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                require(tx, &deal, actor, Capability::Read)?;
                // Event IDs start with their fixed-width UTC timestamp; lexicographic order is (created_at,event_id).
                let mut items = tx.read_with(
                    SqlBuilder::select("deal_activity_events")
                        .columns([
                            "deal_activity_events.event_id",
                            "deal_activity_events.actor_user_id",
                            "deal_activity_events.kind",
                            "deal_activity_events.target_id",
                            "deal_activity_events.created_at",
                            "users.first_name",
                            "users.last_name",
                        ])
                        .left_join("users", "deal_activity_events.actor_user_id", "users.id")
                        .and_where(Condition::equal("deal_id", &deal))
                        .and_where(Condition::compare(
                            "event_id",
                            ComparisonOperator::GreaterThan,
                            after,
                        ))
                        .order_by("event_id", SortDirection::Ascending)
                        .limit(limit + 1)
                        .build(),
                    |r| {
                        let first: Option<String> = r.get("first_name")?;
                        let last: Option<String> = r.get("last_name")?;
                        Ok(Activity {
                            event_id: r.get("event_id")?,
                            actor_user_id: r.get("actor_user_id")?,
                            actor_name: first.zip(last).map(|(a, b)| format!("{a} {b}")),
                            kind: r.get("kind")?,
                            target_id: r.get("target_id")?,
                            created_at: r.get("created_at")?,
                        })
                    },
                )?;
                let next_cursor = if items.len() > limit {
                    items.truncate(limit);
                    items.last().map(|e| e.event_id.clone())
                } else {
                    None
                };
                Ok(Page { items, next_cursor })
            })
            .await
    }
    pub async fn add_member(
        &self,
        deal: String,
        actor: i64,
        email: String,
        role: DealRole,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                let access = require(tx, deal, actor, Capability::ManageMembers)?;
                if role == DealRole::Owner {
                    return Err(RepositoryError::invalid("use ownership transfer"));
                }
                let user = tx
                    .read_one_with(
                        SqlBuilder::select("users")
                            .columns(["id"])
                            .and_where(Condition::equal("email", email))
                            .build(),
                        |r| r.get::<_, i64>(0),
                    )?
                    .ok_or_else(|| RepositoryError::NotFound("registered user not found".into()))?;
                if tx
                    .read_one(
                        SqlBuilder::select("deal_memberships")
                            .and_where(Condition::equal("deal_id", deal))
                            .and_where(Condition::equal("user_id", user))
                            .build(),
                    )?
                    .is_some()
                {
                    return Err(RepositoryError::Conflict("user is already a member".into()));
                }
                let count = tx
                    .read_one_with(
                        SqlBuilder::select("deal_memberships")
                            .count_rows()
                            .and_where(Condition::equal("deal_id", deal))
                            .build(),
                        |r| r.get::<_, i64>(0),
                    )?
                    .unwrap_or(0);
                if count >= 500 {
                    return Err(RepositoryError::Conflict("member limit reached".into()));
                }
                let now = chrono::Utc::now().to_rfc3339();
                tx.write(
                    SqlBuilder::insert_into("deal_memberships")
                        .value("deal_id", deal)
                        .value("user_id", user)
                        .value("role", role.code())
                        .value("added_by_user_id", actor)
                        .value("joined_at", &now)
                        .value("updated_at", now)
                        .build(),
                )?;
                bump(tx, deal, access.access_revision)?;
                event(
                    tx,
                    deal,
                    Some(actor),
                    "member_added",
                    Some(&user.to_string()),
                )?;
                Ok(())
            })
            .await
    }

    pub async fn change_role(
        &self,
        deal: String,
        actor: i64,
        user_id: i64,
        role: DealRole,
        revision: i64,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                let access = require(tx, deal, actor, Capability::ManageMembers)?;
                let (old, actual) = member_role(tx, deal, user_id)?;
                if old == DealRole::Owner || role == DealRole::Owner {
                    return Err(RepositoryError::Forbidden);
                }
                if revision != actual {
                    return Err(RepositoryError::Conflict(
                        "member changed; refresh and retry".into(),
                    ));
                }
                if old == role {
                    return Ok(());
                }
                set_role(tx, deal, user_id, role, actual)?;
                bump(tx, deal, access.access_revision)?;
                event(
                    tx,
                    deal,
                    Some(actor),
                    "member_role_changed",
                    Some(&user_id.to_string()),
                )?;
                Ok(())
            })
            .await
    }

    pub async fn remove_member(
        &self,
        deal: String,
        actor: i64,
        user_id: i64,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                let access = require(
                    tx,
                    deal,
                    actor,
                    if user_id == actor {
                        Capability::Leave
                    } else {
                        Capability::RemoveMember
                    },
                )?;
                if member_role(tx, deal, user_id)?.0 == DealRole::Owner {
                    return Err(RepositoryError::Forbidden);
                }
                tx.write(
                    SqlBuilder::delete_from("deal_memberships")
                        .and_where(Condition::equal("deal_id", deal))
                        .and_where(Condition::equal("user_id", user_id))
                        .build(),
                )?;
                bump(tx, deal, access.access_revision)?;
                event(
                    tx,
                    deal,
                    Some(actor),
                    "member_removed",
                    Some(&user_id.to_string()),
                )?;
                Ok(())
            })
            .await
    }

    pub async fn transfer_ownership(
        &self,
        deal: String,
        actor: i64,
        user_id: i64,
        revision: i64,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                let access = require(tx, deal, actor, Capability::Transfer)?;
                if revision != access.access_revision {
                    return Err(RepositoryError::Conflict(
                        "access changed; refresh and retry".into(),
                    ));
                }
                if user_id == actor {
                    return Err(RepositoryError::invalid("choose another member"));
                }
                let (_, target_revision) = member_role(tx, deal, user_id)?;
                let (_, actor_revision) = member_role(tx, deal, actor)?;
                set_role(tx, deal, actor, DealRole::Manager, actor_revision)?;
                set_role(tx, deal, user_id, DealRole::Owner, target_revision)?;
                bump(tx, deal, access.access_revision)?;
                event(
                    tx,
                    deal,
                    Some(actor),
                    "ownership_transferred",
                    Some(&user_id.to_string()),
                )?;
                Ok(())
            })
            .await
    }

    pub async fn create_team(
        &self,
        deal: String,
        actor: i64,
        name: String,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                require(tx, deal, actor, Capability::ManageTeams)?;
                unique_name(tx, deal, &name, "")?;
                let count = tx
                    .read_one_with(
                        SqlBuilder::select("deal_teams")
                            .count_rows()
                            .and_where(Condition::equal("deal_id", deal))
                            .build(),
                        |r| r.get::<_, i64>(0),
                    )?
                    .unwrap_or(0);
                if count >= 1 {
                    return Err(RepositoryError::Conflict(
                        "a deal can have only one team".into(),
                    ));
                }
                let id = uuid::Uuid::new_v4().to_string();
                let now = chrono::Utc::now().to_rfc3339();
                tx.write(
                    SqlBuilder::insert_into("deal_teams")
                        .value("team_id", &id)
                        .value("deal_id", deal)
                        .value("name", &name)
                        .value("normalized_name", name.to_lowercase())
                        .value("created_by_user_id", actor)
                        .value("created_at", &now)
                        .value("updated_at", &now)
                        .build(),
                )?;
                event(tx, deal, Some(actor), "team_created", Some(&id))?;
                Ok(())
            })
            .await
    }

    pub async fn rename_team(
        &self,
        deal: String,
        actor: i64,
        team_id: String,
        name: String,
        revision: i64,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                require(tx, deal, actor, Capability::ManageTeams)?;
                team_exists(tx, deal, &team_id)?;
                unique_name(tx, deal, &name, &team_id)?;
                if revision >= 9_007_199_254_740_991 {
                    return Err(RepositoryError::Conflict("team revision exhausted".into()));
                }
                let changed = tx.write(
                    SqlBuilder::update("deal_teams")
                        .set("name", &name)
                        .set("normalized_name", name.to_lowercase())
                        .set("revision", revision + 1)
                        .set("updated_at", chrono::Utc::now().to_rfc3339())
                        .and_where(Condition::equal("deal_id", deal))
                        .and_where(Condition::equal("team_id", &team_id))
                        .and_where(Condition::equal("revision", revision))
                        .build(),
                )?;
                if changed.rows_affected == 0 {
                    return Err(RepositoryError::Conflict(
                        "team changed; refresh and retry".into(),
                    ));
                }
                event(tx, deal, Some(actor), "team_renamed", Some(&team_id))?;
                Ok(())
            })
            .await
    }

    pub async fn delete_team(
        &self,
        deal: String,
        actor: i64,
        team_id: String,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                require(tx, deal, actor, Capability::ManageTeams)?;
                team_exists(tx, deal, &team_id)?;
                tx.write(
                    SqlBuilder::delete_from("deal_teams")
                        .and_where(Condition::equal("deal_id", deal))
                        .and_where(Condition::equal("team_id", &team_id))
                        .build(),
                )?;
                event(tx, deal, Some(actor), "team_deleted", Some(&team_id))?;
                Ok(())
            })
            .await
    }

    pub async fn set_team_member(
        &self,
        deal: String,
        actor: i64,
        team_id: String,
        user_id: i64,
        add: bool,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let deal = deal.as_str();
                require(tx, deal, actor, Capability::ManageTeams)?;
                team_exists(tx, deal, &team_id)?;
                member_role(tx, deal, user_id)?;
                let query = || {
                    SqlBuilder::select("deal_team_memberships")
                        .and_where(Condition::equal("deal_id", deal))
                        .and_where(Condition::equal("team_id", &team_id))
                        .and_where(Condition::equal("user_id", user_id))
                        .build()
                };
                if tx.read_one(query())?.is_some() == add {
                    return Ok(());
                }
                if add {
                    tx.write(
                        SqlBuilder::insert_into("deal_team_memberships")
                            .value("deal_id", deal)
                            .value("team_id", &team_id)
                            .value("user_id", user_id)
                            .build(),
                    )?;
                } else {
                    tx.write(
                        SqlBuilder::delete_from("deal_team_memberships")
                            .and_where(Condition::equal("deal_id", deal))
                            .and_where(Condition::equal("team_id", &team_id))
                            .and_where(Condition::equal("user_id", user_id))
                            .build(),
                    )?;
                }
                let kind = if add {
                    "team_member_added"
                } else {
                    "team_member_removed"
                };
                event(
                    tx,
                    deal,
                    Some(actor),
                    kind,
                    Some(&format!("{team_id}/{user_id}")),
                )?;
                Ok(())
            })
            .await
    }
}

pub(crate) fn require(
    tx: &SqliteTransaction<'_>,
    deal: &str,
    actor: i64,
    capability: Capability,
) -> Result<DealAccess, RepositoryError> {
    let row = tx
        .read_one_with(
            SqlBuilder::select("deal_memberships")
                .columns([
                    "deal_memberships.role",
                    "deals.user_id",
                    "deals.archived_at",
                    "deal_access_revisions.revision",
                ])
                .inner_join("deals", "deal_memberships.deal_id", "deals.deal_id")
                .inner_join(
                    "deal_access_revisions",
                    "deals.deal_id",
                    "deal_access_revisions.deal_id",
                )
                .and_where(Condition::equal("deals.deal_id", deal))
                .and_where(Condition::equal("deal_memberships.user_id", actor))
                .build(),
            |r| {
                Ok((
                    role_from_row(r, "role")?,
                    r.get::<_, i64>("user_id")?,
                    r.get::<_, Option<String>>("archived_at")?.is_some(),
                    r.get::<_, i64>("revision")?,
                ))
            },
        )?
        .ok_or_else(|| RepositoryError::NotFound("resource not found".into()))?;
    let (role, creator, archived, revision) = row;
    let access = DealAccess::for_actor(role, creator, archived, revision, actor);
    if !access.permits(capability) {
        return Err(RepositoryError::Forbidden);
    }
    Ok(access)
}
pub(crate) fn event(
    tx: &SqliteTransaction<'_>,
    deal: &str,
    actor: Option<i64>,
    kind: &str,
    target: Option<&str>,
) -> Result<(), SqliteClientError> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
    tx.write(
        SqlBuilder::insert_into("deal_activity_events")
            .value("event_id", format!("{now}_{}", uuid::Uuid::new_v4()))
            .value("deal_id", deal)
            .value("actor_user_id", actor)
            .value("kind", kind)
            .value("target_id", target)
            .value("created_at", now)
            .build(),
    )?;
    Ok(())
}
pub(crate) fn initialize(
    tx: &SqliteTransaction<'_>,
    deal: &str,
    actor: i64,
    now: &str,
) -> Result<(), SqliteClientError> {
    tx.write(
        SqlBuilder::insert_into("deal_memberships")
            .value("deal_id", deal)
            .value("user_id", actor)
            .value("role", "owner")
            .value("added_by_user_id", actor)
            .value("joined_at", now)
            .value("updated_at", now)
            .build(),
    )?;
    tx.write(
        SqlBuilder::insert_into("deal_access_revisions")
            .value("deal_id", deal)
            .build(),
    )?;
    event(tx, deal, Some(actor), "project_created", Some(deal))
}
fn member_role(
    tx: &SqliteTransaction<'_>,
    deal: &str,
    user: i64,
) -> Result<(DealRole, i64), RepositoryError> {
    tx.read_one_with(
        SqlBuilder::select("deal_memberships")
            .and_where(Condition::equal("deal_id", deal))
            .and_where(Condition::equal("user_id", user))
            .build(),
        |r| Ok((role_from_row(r, "role")?, r.get("revision")?)),
    )?
    .ok_or_else(|| RepositoryError::NotFound("member not found".into()))
}
fn bump(tx: &SqliteTransaction<'_>, deal: &str, current: i64) -> Result<(), RepositoryError> {
    let next = current
        .checked_add(1)
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or_else(|| RepositoryError::Conflict("access revision exhausted".into()))?;
    tx.write(
        SqlBuilder::update("deal_access_revisions")
            .set("revision", next)
            .and_where(Condition::equal("deal_id", deal))
            .build(),
    )?;
    Ok(())
}
fn set_role(
    tx: &SqliteTransaction<'_>,
    deal: &str,
    user: i64,
    role: DealRole,
    revision: i64,
) -> Result<(), RepositoryError> {
    if revision >= 9_007_199_254_740_991 {
        return Err(RepositoryError::Conflict(
            "member revision exhausted".into(),
        ));
    }
    tx.write(
        SqlBuilder::update("deal_memberships")
            .set("role", role.code())
            .set("revision", revision + 1)
            .set("updated_at", chrono::Utc::now().to_rfc3339())
            .and_where(Condition::equal("deal_id", deal))
            .and_where(Condition::equal("user_id", user))
            .build(),
    )?;
    Ok(())
}
fn team_exists(tx: &SqliteTransaction<'_>, deal: &str, team: &str) -> Result<(), RepositoryError> {
    if tx
        .read_one(
            SqlBuilder::select("deal_teams")
                .and_where(Condition::equal("deal_id", deal))
                .and_where(Condition::equal("team_id", team))
                .build(),
        )?
        .is_none()
    {
        return Err(RepositoryError::NotFound("team not found".into()));
    }
    Ok(())
}
fn unique_name(
    tx: &SqliteTransaction<'_>,
    deal: &str,
    name: &str,
    except: &str,
) -> Result<(), RepositoryError> {
    if tx
        .read_one(
            SqlBuilder::select("deal_teams")
                .and_where(Condition::equal("deal_id", deal))
                .and_where(Condition::equal("normalized_name", name.to_lowercase()))
                .and_where(Condition::not_equal("team_id", except))
                .build(),
        )?
        .is_some()
    {
        return Err(RepositoryError::Conflict("team name already exists".into()));
    }
    Ok(())
}
fn role_from_row(row: &rusqlite::Row<'_>, column: &str) -> rusqlite::Result<DealRole> {
    let role: String = row.get(column)?;
    match role.as_str() {
        "owner" => Ok(DealRole::Owner),
        "manager" => Ok(DealRole::Manager),
        "contributor" => Ok(DealRole::Contributor),
        "viewer" => Ok(DealRole::Viewer),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}
#[cfg(test)]
#[path = "../../../tests/integration/domains/memberships/repository_tests.rs"]
mod tests;
