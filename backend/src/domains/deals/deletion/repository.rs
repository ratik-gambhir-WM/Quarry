use super::model::{DealDeletion, DeletionPhase, DeletionStatus};
use crate::{
    adapters::sqlite::{
        client::{SqliteClient, SqliteClientError, SqliteTransaction},
        query::{ComparisonOperator as Op, Condition as C, SortDirection, SqlBuilder as Q},
    },
    domains::memberships::{model::Capability, require_deal},
    shared::error::RepositoryError,
};
use rusqlite::Row;

#[derive(Clone)]
pub(crate) struct DeletionRepository {
    pub(super) sqlite: SqliteClient,
}
pub(crate) fn barrier(tx: &SqliteTransaction<'_>, deal: &str) -> Result<bool, SqliteClientError> {
    Ok(tx
        .read_one_with(
            Q::select("deal_deletions")
                .columns(["deletion_id"])
                .and_where(C::equal("deal_id", deal))
                .build(),
            |_| Ok(()),
        )?
        .is_some())
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}
fn decode(r: &Row<'_>) -> rusqlite::Result<DealDeletion> {
    fn enumeration<T: serde::de::DeserializeOwned>(
        r: &Row<'_>,
        column: &str,
    ) -> rusqlite::Result<T> {
        let value: String = r.get(column)?;
        serde_json::from_value(serde_json::Value::String(value)).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
    }
    let deletion_id: String = r.get("deletion_id")?;
    let status = enumeration(r, "status")?;
    Ok(DealDeletion {
        status_url: format!("/api/v1/deal-deletions/{deletion_id}"),
        deletion_id,
        deal_id: r.get("deal_id")?,
        status,
        phase: enumeration(r, "phase")?,
        retryable: matches!(status, DeletionStatus::Retrying | DeletionStatus::Blocked),
        error_code: r.get("error_code")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        completed_at: r.get("completed_at")?,
    })
}
fn find(
    tx: &SqliteTransaction<'_>,
    column: &str,
    id: &str,
) -> Result<Option<(i64, DealDeletion)>, SqliteClientError> {
    tx.read_one_with(
        Q::select("deal_deletions")
            .and_where(C::equal(column, id))
            .build(),
        |r| Ok((r.get("owner_user_id")?, decode(r)?)),
    )
}
fn owned(value: Option<(i64, DealDeletion)>, actor: i64) -> Result<DealDeletion, RepositoryError> {
    value
        .filter(|(owner, _)| *owner == actor)
        .map(|(_, op)| op)
        .ok_or_else(|| RepositoryError::NotFound("resource not found".into()))
}
impl DeletionRepository {
    pub(crate) fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }
    pub(super) async fn preflight(&self, deal: String, actor: i64) -> Result<(), RepositoryError> {
        self.sqlite
            .read_transaction_result_async(move |tx| {
                if let Some(receipt) = find(tx, "deal_id", &deal)? {
                    owned(Some(receipt), actor)?;
                } else {
                    require_deal(tx, &deal, actor, Capability::Delete)?;
                }
                Ok(())
            })
            .await
    }
    pub(super) async fn accept(
        &self,
        deal: String,
        actor: i64,
    ) -> Result<DealDeletion, RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                if let Some(receipt) = find(tx, "deal_id", &deal)? {
                    let op = owned(Some(receipt), actor)?;
                    if matches!(
                        op.status,
                        DeletionStatus::Blocked | DeletionStatus::Retrying
                    ) {
                        tx.write(
                            Q::update("deal_deletions")
                                .set("status", "pending")
                                .set("next_attempt", 0)
                                .set("error_code", Option::<String>::None)
                                .set("updated_at", now())
                                .and_where(C::equal("deletion_id", &op.deletion_id))
                                .build(),
                        )?;
                        return owned(find(tx, "deletion_id", &op.deletion_id)?, actor);
                    }
                    return Ok(op);
                }
                require_deal(tx, &deal, actor, Capability::Delete)?;
                let id = uuid::Uuid::new_v4().to_string();
                let time = now();
                tx.write(
                    Q::insert_into("deal_deletions")
                        .value("deletion_id", &id)
                        .value("deal_id", &deal)
                        .value("owner_user_id", actor)
                        .value("status", "pending")
                        .value("phase", "draining")
                        .value("created_at", &time)
                        .value("updated_at", &time)
                        .build(),
                )?;
                owned(find(tx, "deletion_id", &id)?, actor)
            })
            .await
    }
    pub(super) async fn status(
        &self,
        id: String,
        actor: i64,
    ) -> Result<DealDeletion, RepositoryError> {
        self.sqlite
            .read_transaction_result_async(move |tx| owned(find(tx, "deletion_id", &id)?, actor))
            .await
    }
    pub(super) async fn pending(&self, after: &str) -> Result<Vec<DealDeletion>, RepositoryError> {
        Ok(self
            .sqlite
            .read_with_async(
                Q::select("deal_deletions")
                    .and_where(C::in_values(
                        "status",
                        vec!["pending", "running", "retrying"],
                    ))
                    .and_where(C::compare(
                        "next_attempt",
                        Op::LessThanOrEqual,
                        chrono::Utc::now().timestamp(),
                    ))
                    .and_where(C::compare("deletion_id", Op::GreaterThan, after))
                    .order_by("deletion_id", SortDirection::Ascending)
                    .limit(16)
                    .build(),
                decode,
            )
            .await?)
    }
    /// Include delayed retries; an empty due page alone does not mean the queue is drained.
    pub(super) async fn has_pending(&self) -> Result<bool, RepositoryError> {
        Ok(self
            .sqlite
            .read_one_with_async(
                Q::select("deal_deletions")
                    .columns(["deletion_id"])
                    .and_where(C::in_values("status", ["pending", "running", "retrying"]))
                    .limit(1)
                    .build(),
                |_| Ok(()),
            )
            .await?
            .is_some())
    }
    pub(super) async fn checkpoint(
        &self,
        id: String,
        phase: DeletionPhase,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .write_async(
                Q::update("deal_deletions")
                    .set("status", "running")
                    .set("phase", phase.code())
                    .set("error_code", Option::<String>::None)
                    .set("updated_at", now())
                    .and_where(C::equal("deletion_id", id))
                    .build(),
            )
            .await?;
        Ok(())
    }
    pub(super) async fn snapshot_and_purge(&self, op: DealDeletion) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let (owner, saved) = find(tx, "deletion_id", &op.deletion_id)?
                    .ok_or_else(|| RepositoryError::NotFound("deletion not found".into()))?;
                if matches!(
                    saved.phase,
                    DeletionPhase::Graph | DeletionPhase::Verifying | DeletionPhase::Completed
                ) {
                    return Ok(());
                }
                if tx
                    .read_one_with(
                        Q::select("deal_memberships")
                            .columns(["user_id"])
                            .and_where(C::equal("deal_id", &op.deal_id))
                            .and_where(C::equal("user_id", owner))
                            .and_where(C::equal("role", "owner"))
                            .build(),
                        |_| Ok(()),
                    )?
                    .is_none()
                {
                    return Err(RepositoryError::invalid(
                        "deletion ownership checkpoint changed",
                    ));
                }
                for (source, column, target) in [
                    ("quarry_files", "file_id", "deal_deletion_files"),
                    ("assistant_threads", "thread_id", "deal_deletion_threads"),
                ] {
                    let mut after = String::new();
                    loop {
                        let ids = tx.read_with(
                            Q::select(source)
                                .columns([column])
                                .and_where(C::equal("deal_id", &op.deal_id))
                                .and_where(C::compare(column, Op::GreaterThan, after.clone()))
                                .order_by(column, SortDirection::Ascending)
                                .limit(512)
                                .build(),
                            |r| r.get::<_, String>(0),
                        )?;
                        if ids.is_empty() {
                            break;
                        }
                        for id in &ids {
                            tx.write(
                                Q::insert_into(target)
                                    .value("deletion_id", &op.deletion_id)
                                    .value(column, id)
                                    .on_conflict_do_nothing(["deletion_id", column])
                                    .build(),
                            )?;
                        }
                        after = ids.last().cloned().unwrap_or_default();
                    }
                }
                tx.write(
                    Q::delete_from("assistant_threads")
                        .and_where(C::equal("deal_id", &op.deal_id))
                        .build(),
                )?;
                tx.write(
                    Q::update("notebooks")
                        .set("deal_id", Option::<String>::None)
                        .set("updated_at", now())
                        .and_where(C::equal("deal_id", &op.deal_id))
                        .build(),
                )?;
                tx.write(
                    Q::delete_from("deals")
                        .and_where(C::equal("deal_id", &op.deal_id))
                        .build(),
                )?;
                tx.write(
                    Q::update("deal_deletions")
                        .set("status", "running")
                        .set("phase", "graph")
                        .set("updated_at", now())
                        .and_where(C::equal("deletion_id", op.deletion_id))
                        .build(),
                )?;
                Ok(())
            })
            .await
    }
    pub(super) async fn files(&self, id: String) -> Result<Vec<String>, RepositoryError> {
        let q = Q::select("deal_deletion_files")
            .columns(["file_id"])
            .and_where(C::equal("deletion_id", id))
            .and_where(C::equal("purged", 0));
        Ok(self
            .sqlite
            .read_with_async(
                q.order_by("file_id", SortDirection::Ascending)
                    .limit(64)
                    .build(),
                |r| r.get(0),
            )
            .await?)
    }
    pub(super) async fn purged_file(
        &self,
        id: String,
        file: String,
    ) -> Result<(), RepositoryError> {
        self.sqlite
            .write_async(
                Q::update("deal_deletion_files")
                    .set("purged", 1)
                    .and_where(C::equal("deletion_id", id))
                    .and_where(C::equal("file_id", file))
                    .build(),
            )
            .await?;
        Ok(())
    }
    pub(super) async fn failure(&self, id: String, blocked: bool) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let attempts: i64 = tx
                    .read_one_with(
                        Q::select("deal_deletions")
                            .columns(["attempts"])
                            .and_where(C::equal("deletion_id", &id))
                            .build(),
                        |r| r.get(0),
                    )?
                    .unwrap_or(0);
                let delay = 2_i64
                    .pow(u32::try_from(attempts.min(8)).unwrap_or(8))
                    .min(300);
                tx.write(
                    Q::update("deal_deletions")
                        .set("status", if blocked { "blocked" } else { "retrying" })
                        .set("attempts", attempts.saturating_add(1))
                        .set(
                            "next_attempt",
                            chrono::Utc::now().timestamp().saturating_add(delay),
                        )
                        .set(
                            "error_code",
                            if blocked {
                                "deletion_integrity_failed"
                            } else {
                                "cleanup_unavailable"
                            },
                        )
                        .set("updated_at", now())
                        .and_where(C::equal("deletion_id", id))
                        .build(),
                )?;
                Ok(())
            })
            .await
    }
    pub(super) async fn complete(&self, id: String) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let op = find(tx, "deletion_id", &id)?
                    .ok_or_else(|| RepositoryError::NotFound("deletion not found".into()))?
                    .1;
                for (table, column, value) in [
                    ("deals", "deal_id", op.deal_id.as_str()),
                    ("quarry_files", "deal_id", op.deal_id.as_str()),
                    ("assistant_threads", "deal_id", op.deal_id.as_str()),
                    ("notebooks", "deal_id", op.deal_id.as_str()),
                ] {
                    if tx
                        .read_one_with(
                            Q::select(table)
                                .columns([column])
                                .and_where(C::equal(column, value))
                                .limit(1)
                                .build(),
                            |_| Ok(()),
                        )?
                        .is_some()
                    {
                        return Err(RepositoryError::Conflict(
                            "deletion verification failed".into(),
                        ));
                    }
                }
                if tx
                    .read_one_with(
                        Q::select("deal_deletion_files")
                            .columns(["file_id"])
                            .and_where(C::equal("deletion_id", &id))
                            .and_where(C::equal("purged", 0))
                            .limit(1)
                            .build(),
                        |_| Ok(()),
                    )?
                    .is_some()
                {
                    return Err(RepositoryError::Conflict("deletion targets remain".into()));
                }
                for table in ["deal_deletion_files", "deal_deletion_threads"] {
                    tx.write(
                        Q::delete_from(table)
                            .and_where(C::equal("deletion_id", &id))
                            .build(),
                    )?;
                }
                let time = now();
                tx.write(
                    Q::update("deal_deletions")
                        .set("status", DeletionStatus::Completed.code())
                        .set("phase", "completed")
                        .set("error_code", Option::<String>::None)
                        .set("updated_at", &time)
                        .set("completed_at", &time)
                        .and_where(C::equal("deletion_id", id))
                        .build(),
                )?;
                Ok(())
            })
            .await
    }
}

pub(super) fn integrity_failure(error: &RepositoryError) -> bool {
    match error {
        RepositoryError::InvalidData(_) | RepositoryError::Conflict(_) | RepositoryError::NotFound(_) => true,
        RepositoryError::StorageCause(source) => {
            source.downcast_ref::<crate::adapters::sqlite::client::SqliteClientError>().is_some_and(|error| matches!(error, crate::adapters::sqlite::client::SqliteClientError::Database(rusqlite::Error::SqliteFailure(code, _)) if code.code == rusqlite::ErrorCode::ConstraintViolation))
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/deals/deletion/repository_tests.rs"]
mod tests;
