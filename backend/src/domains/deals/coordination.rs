//! Lock deal authority before reading it; ordinary reads never enter this protocol.
use crate::{
    adapters::postgres::{
        client::PostgresTransaction,
        query::{Condition, LockWait, RowLock, SqlBuilder},
    },
    shared::error::RepositoryError,
};

/// Retain the chosen mode through canonical commit. Choose Update immediately for
/// authority/lifecycle changes, avoiding a Share-to-Update lock upgrade.
pub(crate) async fn lock(
    tx: &PostgresTransaction,
    deal: &str,
    mode: RowLock,
) -> Result<String, RepositoryError> {
    tx.lock_with(
        SqlBuilder::select("deal_lifecycle_guards")
            .and_where(Condition::equal("deal_id", deal))
            .lock(mode, LockWait::Wait)
            .build(),
        |r| r.get::<_, String>("state"),
    )
    .await?
    .pop()
    .ok_or_else(|| RepositoryError::NotFound("resource not found".into()))
}

/// Acquire the guard before policy evaluation. The owning policy checks state
/// after resolving the actor, preserving inaccessible-resource disclosure rules.
pub(crate) async fn lock_authority(
    tx: &PostgresTransaction,
    deal: &str,
    mode: RowLock,
) -> Result<(), RepositoryError> {
    lock(tx, deal, mode).await?;
    Ok(())
}
