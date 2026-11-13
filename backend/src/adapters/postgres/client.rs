//! Bounded asynchronous PostgreSQL I/O. All aggregate writes share a database-scoped
//! transaction advisory lock; readers use repeatable-read, read-only snapshots.
//! The conservative write ordering preserves the old aggregate invariants across
//! processes without assuming that a connection pool serializes writers. Reads
//! depend only on their pooled connection, never on application writer ownership.
use super::query::{QueryKind, SqlBuilderError, SqlQuery};
pub use super::row::{Row, SqlRow};
pub type RowResult<T> = Result<T, PostgresError>;
use crate::app::config::PostgresConfig;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, PgPool, Postgres, Transaction,
};
use std::{future::Future, sync::Arc};
use thiserror::Error;
use tokio::sync::Mutex;

pub const WRITE_LOCK: i64 = 0x517561727279;
const WRITER_GENERATION_KEY: &str = "postgres_writer_generation";
#[derive(Debug, Error)]
pub enum PostgresError {
    #[error("PostgreSQL query construction failed: {0}")]
    QueryBuild(#[from] SqlBuilderError),
    #[error("PostgreSQL storage operation failed")]
    Database(#[from] sqlx::Error),
    #[error("another Quarry writer holds the selected PostgreSQL schema")]
    WriterBusy,
    #[error("PostgreSQL writer ownership was lost; restart Quarry before continuing")]
    WriterLeaseLost,
    #[error("PostgreSQL row decoding failed: {0}")]
    Decode(String),
    #[error("cannot use a {kind:?} query for a {operation} operation")]
    InvalidOperation {
        operation: &'static str,
        kind: QueryKind,
    },
    #[error("transaction was aborted: {0}")]
    TransactionAborted(String),
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<PostgresError>,
    },
}
impl PostgresError {
    pub fn transaction_aborted(message: impl Into<String>) -> Self {
        Self::TransactionAborted(message.into())
    }
    pub fn decode(message: impl std::fmt::Display) -> Self {
        Self::Decode(message.to_string())
    }
    pub fn context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
    pub fn is_integrity_violation(&self) -> bool {
        match self {
            Self::Database(sqlx::Error::Database(e)) => {
                e.code().is_some_and(|c| c.starts_with("23"))
            }
            Self::Context { source, .. } => source.is_integrity_violation(),
            _ => false,
        }
    }
    pub fn is_unique_violation(&self) -> bool {
        match self {
            Self::Database(sqlx::Error::Database(e)) => e.is_unique_violation(),
            Self::Context { source, .. } => source.is_unique_violation(),
            _ => false,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteResult {
    pub rows_affected: usize,
    pub inserted_id: i64,
}
#[derive(Clone)]
pub struct PostgresClient {
    pool: PgPool,
    writer_lease: Option<Arc<WriterLease>>,
}
struct WriterLease {
    connection: Mutex<Option<sqlx::pool::PoolConnection<Postgres>>>,
    healthy: std::sync::atomic::AtomicBool,
    generation: String,
}
pub struct PostgresTransaction {
    connection: Arc<Mutex<Option<Transaction<'static, Postgres>>>>,
    read_only: bool,
}
fn bind(query: &SqlQuery) -> sqlx::query::Query<'_, Postgres, sqlx::postgres::PgArguments> {
    let mut q = sqlx::query(query.sql());
    for value in query.parameters() {
        q = q.bind(value);
    }
    q
}
impl PostgresClient {
    pub async fn connect(config: &PostgresConfig) -> Result<Self, PostgresError> {
        let options = config
            .url
            .expose()
            .parse::<PgConnectOptions>()
            .map_err(|_| PostgresError::transaction_aborted("invalid database configuration"))?
            .disable_statement_logging();
        let statement_ms = config.statement_timeout.as_millis().to_string();
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.acquire_timeout)
            .after_connect(move |conn, _| {
                let ms = statement_ms.clone();
                Box::pin(async move {
                    sqlx::query(
                        "SELECT set_config('statement_timeout', $1, false),
                                set_config('lock_timeout', $1, false),
                                set_config('idle_in_transaction_session_timeout', $1, false),
                                set_config('TimeZone', 'UTC', false)",
                    )
                    .bind(ms)
                    .execute(conn)
                    .await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await?;
        Ok(Self {
            pool,
            writer_lease: None,
        })
    }
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }
    /// Retain the single active application writer through routers and detached workers.
    /// The dedicated pooled connection closes on drop so session locks never return to the pool.
    /// The schema must be initialized before registering the commit fence.
    pub(crate) async fn with_writer_lease(mut self) -> Result<Self, PostgresError> {
        let mut connection = self.pool.acquire().await?;
        connection.close_on_drop();
        let owned: bool = sqlx::query_scalar(
            "SELECT pg_try_advisory_lock(hashtextextended(current_schema(),$1))",
        )
        .bind(WRITE_LOCK + 1)
        .fetch_one(&mut *connection)
        .await?;
        if !owned {
            return Err(PostgresError::WriterBusy);
        }
        let generation = uuid::Uuid::new_v4().to_string();
        // This ownership update deliberately bypasses the aggregate write lock: a new
        // owner must fence an old transaction that is still holding that lock. A
        // transaction already at its commit fence holds this row until it finishes,
        // so takeover cannot return before that commit/rollback is resolved.
        sqlx::query(
            "INSERT INTO app_metadata(key,value) VALUES ($1,$2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        )
        .bind(WRITER_GENERATION_KEY)
        .bind(&generation)
        .execute(&mut *connection)
        .await?;
        self.writer_lease = Some(Arc::new(WriterLease {
            connection: Mutex::new(Some(connection)),
            healthy: std::sync::atomic::AtomicBool::new(true),
            generation,
        }));
        Ok(self)
    }
    async fn ensure_writer_lease(&self) -> Result<(), PostgresError> {
        use std::sync::atomic::Ordering;
        if let Some(lease) = &self.writer_lease {
            if !lease.healthy.load(Ordering::Acquire) {
                return Err(PostgresError::WriterLeaseLost);
            }
            let mut guard = lease.connection.lock().await;
            let Some(connection) = guard.as_mut() else {
                return Err(PostgresError::WriterLeaseLost);
            };
            if sqlx::query("SELECT 1")
                .execute(&mut **connection)
                .await
                .is_err()
            {
                lease.healthy.store(false, Ordering::Release);
                return Err(PostgresError::WriterLeaseLost);
            }
        }
        Ok(())
    }
    async fn fence_commit(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), PostgresError> {
        self.ensure_writer_lease().await?;
        if let Some(lease) = &self.writer_lease {
            // READ COMMITTED sees a replacement owner's generation, even if this
            // transaction started earlier. Keep the row locked through commit to
            // close the gap between checking ownership and publishing the write.
            let generation: Option<String> =
                sqlx::query_scalar("SELECT value FROM app_metadata WHERE key=$1 FOR SHARE")
                    .bind(WRITER_GENERATION_KEY)
                    .fetch_optional(&mut **transaction)
                    .await?;
            if generation.as_deref() != Some(lease.generation.as_str()) {
                lease
                    .healthy
                    .store(false, std::sync::atomic::Ordering::Release);
                return Err(PostgresError::WriterLeaseLost);
            }
        }
        Ok(())
    }
    pub async fn close(&self) {
        if let Some(lease) = &self.writer_lease {
            lease
                .healthy
                .store(false, std::sync::atomic::Ordering::Release);
            drop(lease.connection.lock().await.take());
        }
        self.pool.close().await;
    }
    async fn execute_transaction<T, E, F, Fut>(
        &self,
        transaction: Transaction<'static, Postgres>,
        read_only: bool,
        operation: F,
    ) -> Result<T, E>
    where
        E: From<PostgresError>,
        F: FnOnce(PostgresTransaction) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let connection = Arc::new(Mutex::new(Some(transaction)));
        let result = operation(PostgresTransaction {
            connection: connection.clone(),
            read_only,
        })
        .await;
        let mut transaction =
            connection.lock().await.take().ok_or_else(|| {
                PostgresError::transaction_aborted("transaction already finished")
            })?;
        let result = match result {
            Ok(result) if !read_only => self
                .fence_commit(&mut transaction)
                .await
                .map(|()| result)
                .map_err(E::from),
            Ok(result) => Ok(result),
            Err(error) => Err(error),
        };
        match result {
            Ok(result) => {
                transaction.commit().await.map_err(PostgresError::from)?;
                Ok(result)
            }
            Err(error) => {
                if let Err(rollback) = transaction.rollback().await {
                    tracing::error!(error=%rollback,"PostgreSQL rollback failed");
                }
                Err(error)
            }
        }
    }
    pub async fn transaction_result_async<T, E, F, Fut>(&self, op: F) -> Result<T, E>
    where
        E: From<PostgresError>,
        F: FnOnce(PostgresTransaction) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        self.ensure_writer_lease().await?;
        let mut transaction = self.pool.begin().await.map_err(PostgresError::from)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),$1))")
            .bind(WRITE_LOCK)
            .execute(&mut *transaction)
            .await
            .map_err(PostgresError::from)?;
        // Pool acquisition and the aggregate lock may have waited through takeover.
        self.ensure_writer_lease().await?;
        self.execute_transaction(transaction, false, op).await
    }
    pub async fn transaction_async<T, F, Fut>(&self, op: F) -> Result<T, PostgresError>
    where
        F: FnOnce(PostgresTransaction) -> Fut,
        Fut: Future<Output = Result<T, PostgresError>>,
    {
        self.transaction_result_async(op).await
    }
    pub async fn read_transaction_result_async<T, E, F, Fut>(&self, op: F) -> Result<T, E>
    where
        E: From<PostgresError>,
        F: FnOnce(PostgresTransaction) -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        let mut transaction = self.pool.begin().await.map_err(PostgresError::from)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(PostgresError::from)?;
        self.execute_transaction(transaction, true, op).await
    }
    pub async fn read_transaction_async<T, F, Fut>(&self, op: F) -> Result<T, PostgresError>
    where
        F: FnOnce(PostgresTransaction) -> Fut,
        Fut: Future<Output = Result<T, PostgresError>>,
    {
        self.read_transaction_result_async(op).await
    }
    pub async fn read_with_async<T>(
        &self,
        query: Result<SqlQuery, PostgresError>,
        mut mapper: impl FnMut(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<Vec<T>, PostgresError> {
        let query = query?;
        ensure(&query, true)?;
        let rows = bind(&query).fetch_all(&self.pool).await?;
        rows.iter().map(|r| mapper(&Row(r))).collect()
    }

    pub async fn read_one_with_async<T>(
        &self,
        query: Result<SqlQuery, PostgresError>,
        mapper: impl FnOnce(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<Option<T>, PostgresError> {
        let query = query?;
        ensure(&query, true)?;
        let row = bind(&query).fetch_optional(&self.pool).await?;
        row.as_ref().map(|r| mapper(&Row(r))).transpose()
    }
    pub async fn read_async(
        &self,
        query: Result<SqlQuery, PostgresError>,
    ) -> Result<Vec<SqlRow>, PostgresError> {
        self.read_with_async(query, |row| row.owned()).await
    }
    pub async fn read_one_async(
        &self,
        query: Result<SqlQuery, PostgresError>,
    ) -> Result<Option<SqlRow>, PostgresError> {
        self.read_one_with_async(query, |row| row.owned()).await
    }
    pub async fn write_async(
        &self,
        query: Result<SqlQuery, PostgresError>,
    ) -> Result<WriteResult, PostgresError> {
        let query = query?;
        ensure(&query, false)?;
        self.transaction_async(|tx| async move { tx.write(Ok(query)).await })
            .await
    }
}
impl PostgresTransaction {
    pub async fn read_result_with<T, E>(
        &self,
        query: Result<SqlQuery, PostgresError>,
        mapper: impl FnMut(&Row<'_>) -> Result<T, E>,
    ) -> Result<Vec<T>, E>
    where
        E: From<PostgresError>,
    {
        let query = query?;
        ensure(&query, true)?;
        self.rows_with(query, mapper).await
    }

    /// Lock only the resources selected by a typed locking query in this transaction.
    /// Read-only transactions reject this operation before executing SQL.
    ///
    /// # Errors
    /// Returns an error for a nonlocking query, a read-only transaction, or a
    /// database/row-mapping failure. Lock waits retain the configured timeout.
    pub async fn lock_with<T>(
        &self,
        query: Result<SqlQuery, PostgresError>,
        mapper: impl FnMut(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<Vec<T>, PostgresError> {
        let query = query?;
        if query.kind() != QueryKind::LockingSelect {
            return Err(PostgresError::InvalidOperation {
                operation: "row lock",
                kind: query.kind(),
            });
        }
        self.ensure_writable()?;
        self.rows_with(query, mapper).await
    }

    /// Return rows from an INSERT or an atomic UPDATE/DELETE predicate.
    /// An empty result is a failed predicate, never an implicit successful edit.
    ///
    /// # Errors
    /// Returns an error for a query without mutation RETURNING metadata, a
    /// read-only transaction, or a database/row-mapping failure.
    pub async fn write_returning_with<T>(
        &self,
        query: Result<SqlQuery, PostgresError>,
        mapper: impl FnMut(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<Vec<T>, PostgresError> {
        let query = query?;
        if !query.kind().returns_mutation_rows() {
            return Err(PostgresError::InvalidOperation {
                operation: "returning mutation",
                kind: query.kind(),
            });
        }
        self.ensure_writable()?;
        self.rows_with(query, mapper).await
    }

    async fn rows_with<T, E>(
        &self,
        query: SqlQuery,
        mut mapper: impl FnMut(&Row<'_>) -> Result<T, E>,
    ) -> Result<Vec<T>, E>
    where
        E: From<PostgresError>,
    {
        let mut guard = self.connection.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| PostgresError::transaction_aborted("transaction finished"))?;
        let rows = bind(&query)
            .fetch_all(&mut **tx)
            .await
            .map_err(PostgresError::from)?;
        rows.iter().map(|r| mapper(&Row(r))).collect()
    }
    pub async fn read_with<T>(
        &self,
        query: Result<SqlQuery, PostgresError>,
        mapper: impl FnMut(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<Vec<T>, PostgresError> {
        self.read_result_with(query, mapper).await
    }
    pub async fn read_one_with<T>(
        &self,
        query: Result<SqlQuery, PostgresError>,
        mapper: impl FnOnce(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<Option<T>, PostgresError> {
        let query = query?;
        ensure(&query, true)?;
        let mut guard = self.connection.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| PostgresError::transaction_aborted("transaction finished"))?;
        bind(&query)
            .fetch_optional(&mut **tx)
            .await?
            .as_ref()
            .map(|r| mapper(&Row(r)))
            .transpose()
    }
    pub async fn read(
        &self,
        query: Result<SqlQuery, PostgresError>,
    ) -> Result<Vec<SqlRow>, PostgresError> {
        self.read_with(query, |row| row.owned()).await
    }
    pub async fn read_one(
        &self,
        query: Result<SqlQuery, PostgresError>,
    ) -> Result<Option<SqlRow>, PostgresError> {
        self.read_one_with(query, |row| row.owned()).await
    }
    pub async fn write(
        &self,
        query: Result<SqlQuery, PostgresError>,
    ) -> Result<WriteResult, PostgresError> {
        let query = query?;
        ensure(&query, false)?;
        self.ensure_writable()?;
        let mut guard = self.connection.lock().await;
        let tx = guard
            .as_mut()
            .ok_or_else(|| PostgresError::transaction_aborted("transaction finished"))?;
        if query.kind() == QueryKind::InsertReturningId {
            let row = bind(&query).fetch_optional(&mut **tx).await?;
            Ok(WriteResult {
                rows_affected: usize::from(row.is_some()),
                inserted_id: row
                    .as_ref()
                    .map(|r| Row(r).get(0))
                    .transpose()?
                    .unwrap_or(0),
            })
        } else {
            let result = bind(&query).execute(&mut **tx).await?;
            Ok(WriteResult {
                rows_affected: usize::try_from(result.rows_affected())
                    .map_err(PostgresError::decode)?,
                inserted_id: 0,
            })
        }
    }

    fn ensure_writable(&self) -> Result<(), PostgresError> {
        if self.read_only {
            return Err(PostgresError::transaction_aborted("read-only transaction"));
        }
        Ok(())
    }
}
fn ensure(query: &SqlQuery, read: bool) -> Result<(), PostgresError> {
    let allowed = if read {
        query.kind().is_read()
    } else {
        query.kind().is_mutation()
    };
    if !allowed {
        return Err(PostgresError::InvalidOperation {
            operation: if read { "read" } else { "write" },
            kind: query.kind(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/integration/adapters/postgres/concurrency_tests.rs"]
mod concurrency_tests;
#[cfg(test)]
#[path = "../../../tests/integration/adapters/postgres/read_tests.rs"]
mod read_tests;
#[cfg(test)]
#[path = "../../../tests/integration/adapters/postgres/support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "../../../tests/integration/adapters/postgres/client_tests.rs"]
mod tests;
