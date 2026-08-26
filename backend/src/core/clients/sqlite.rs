use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use rusqlite::{
    params_from_iter, types::ValueRef, Connection, OptionalExtension, Row, Transaction,
    TransactionBehavior,
};
use thiserror::Error;

use crate::core::sqlbuilder::{QueryKind, SqlQuery, SqlValue};

const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Error)]
pub enum SqliteClientError {
    #[error("sqlite connection lock was poisoned")]
    ConnectionLockPoisoned,
    #[error("sqlite query failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("cannot use a {kind:?} query for a {operation} operation")]
    InvalidOperation {
        operation: &'static str,
        kind: QueryKind,
    },
    #[error("sqlite blocking worker failed: {0}")]
    BlockingWorker(String),
    #[error("sqlite transaction was aborted: {0}")]
    TransactionAborted(String),
    #[error("{context}: {source}")]
    Context {
        context: String,
        #[source]
        source: Box<SqliteClientError>,
    },
    #[error(
        "sqlite transaction failed and rollback also failed: {rollback}; original error: {source}"
    )]
    TransactionRollback {
        #[source]
        source: Box<SqliteClientError>,
        rollback: rusqlite::Error,
    },
}

impl SqliteClientError {
    pub fn transaction_aborted(message: impl Into<String>) -> Self {
        Self::TransactionAborted(message.into())
    }

    pub fn context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlRow {
    values: HashMap<String, SqlValue>,
}

impl SqlRow {
    pub fn get(&self, column: &str) -> Option<&SqlValue> {
        self.values.get(column)
    }

    pub fn values(&self) -> &HashMap<String, SqlValue> {
        &self.values
    }

    pub fn into_values(self) -> HashMap<String, SqlValue> {
        self.values
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteResult {
    pub rows_affected: usize,
    pub last_insert_row_id: i64,
}

/// Query access scoped to one SQLite transaction and one connection.
///
/// The raw rusqlite transaction stays inside the client layer. Callers can
/// only execute validated `SqlQuery` values and cannot commit independently.
pub struct SqliteTransaction<'connection> {
    transaction: Transaction<'connection>,
}

impl SqliteTransaction<'_> {
    pub fn read(&self, query: &SqlQuery) -> Result<Vec<SqlRow>, SqliteClientError> {
        ensure_read_query(query)?;
        let mut statement = self.transaction.prepare(query.sql())?;
        let rows = statement
            .query_map(params_from_iter(query.parameters()), sql_row_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn read_one(&self, query: &SqlQuery) -> Result<Option<SqlRow>, SqliteClientError> {
        ensure_read_query(query)?;
        Ok(self
            .transaction
            .query_row(
                query.sql(),
                params_from_iter(query.parameters()),
                sql_row_from_row,
            )
            .optional()?)
    }

    pub fn write(&self, query: &SqlQuery) -> Result<WriteResult, SqliteClientError> {
        ensure_write_query(query)?;
        let rows_affected = self
            .transaction
            .execute(query.sql(), params_from_iter(query.parameters()))?;
        Ok(WriteResult {
            rows_affected,
            last_insert_row_id: self.transaction.last_insert_rowid(),
        })
    }
}

/// Cloneable access to one SQLite connection.
///
/// Synchronous methods are useful inside an existing `spawn_blocking` task.
/// The `_async` variants move the database work to Tokio's blocking pool so an
/// Axum handler does not block a runtime worker.
#[derive(Clone)]
pub struct SqliteClient {
    connection: Arc<Mutex<Connection>>,
    path: Arc<PathBuf>,
}

impl SqliteClient {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteClientError> {
        let path = path.as_ref().to_path_buf();
        let connection = Connection::open(&path)?;
        Self::from_connection(connection, path)
    }

    pub fn open_in_memory() -> Result<Self, SqliteClientError> {
        Self::from_connection(Connection::open_in_memory()?, PathBuf::from(":memory:"))
    }

    pub fn from_connection(
        connection: Connection,
        path: impl Into<PathBuf>,
    ) -> Result<Self, SqliteClientError> {
        connection.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            path: Arc::new(path.into()),
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> rusqlite::Result<T>,
    ) -> Result<T, SqliteClientError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqliteClientError::ConnectionLockPoisoned)?;
        let result = operation(&mut connection)?;
        Ok(result)
    }

    pub(crate) fn with_connection_result<T, E>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<SqliteClientError>,
    {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| E::from(SqliteClientError::ConnectionLockPoisoned))?;
        operation(&mut connection)
    }

    pub fn read(&self, query: &SqlQuery) -> Result<Vec<SqlRow>, SqliteClientError> {
        self.read_with(query, sql_row_from_row)
    }

    pub fn read_one(&self, query: &SqlQuery) -> Result<Option<SqlRow>, SqliteClientError> {
        self.read_one_with(query, sql_row_from_row)
    }

    pub fn read_with<T, F>(
        &self,
        query: &SqlQuery,
        mut map_row: F,
    ) -> Result<Vec<T>, SqliteClientError>
    where
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        ensure_read_query(query)?;
        self.with_connection(|connection| {
            let mut statement = connection.prepare(query.sql())?;
            let rows = statement
                .query_map(params_from_iter(query.parameters()), |row| map_row(row))?
                .collect();
            rows
        })
    }

    pub fn read_one_with<T, F>(
        &self,
        query: &SqlQuery,
        mut map_row: F,
    ) -> Result<Option<T>, SqliteClientError>
    where
        F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
    {
        ensure_read_query(query)?;
        self.with_connection(|connection| {
            connection
                .query_row(query.sql(), params_from_iter(query.parameters()), |row| {
                    map_row(row)
                })
                .optional()
        })
    }

    pub fn write(&self, query: &SqlQuery) -> Result<WriteResult, SqliteClientError> {
        ensure_write_query(query)?;
        self.with_connection(|connection| {
            let rows_affected =
                connection.execute(query.sql(), params_from_iter(query.parameters()))?;
            Ok(WriteResult {
                rows_affected,
                last_insert_row_id: connection.last_insert_rowid(),
            })
        })
    }

    /// Runs one `BEGIN IMMEDIATE` transaction while holding the connection
    /// lock exactly once. The closure is synchronous so the transaction can
    /// never be held across an `.await`.
    pub fn transaction<T, F>(&self, operation: F) -> Result<T, SqliteClientError>
    where
        F: for<'connection> FnOnce(&SqliteTransaction<'connection>) -> Result<T, SqliteClientError>,
    {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqliteClientError::ConnectionLockPoisoned)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(SqliteClientError::from)
            .map_err(|error| error.context("failed to begin immediate sqlite transaction"))?;
        let transaction = SqliteTransaction { transaction };
        let result = operation(&transaction);

        match result {
            Ok(value) => {
                transaction
                    .transaction
                    .commit()
                    .map_err(SqliteClientError::from)
                    .map_err(|error| error.context("failed to commit sqlite transaction"))?;
                Ok(value)
            }
            Err(error) => match transaction.transaction.rollback() {
                Ok(()) => Err(error),
                Err(rollback) => Err(SqliteClientError::TransactionRollback {
                    source: Box::new(error),
                    rollback,
                }),
            },
        }
    }

    pub async fn read_async(&self, query: SqlQuery) -> Result<Vec<SqlRow>, SqliteClientError> {
        let client = self.clone();
        run_blocking(move || client.read(&query)).await
    }

    pub async fn read_one_async(
        &self,
        query: SqlQuery,
    ) -> Result<Option<SqlRow>, SqliteClientError> {
        let client = self.clone();
        run_blocking(move || client.read_one(&query)).await
    }

    pub async fn read_with_async<T, F>(
        &self,
        query: SqlQuery,
        map_row: F,
    ) -> Result<Vec<T>, SqliteClientError>
    where
        T: Send + 'static,
        F: for<'row> FnMut(&Row<'row>) -> rusqlite::Result<T> + Send + 'static,
    {
        let client = self.clone();
        run_blocking(move || client.read_with(&query, map_row)).await
    }

    pub async fn read_one_with_async<T, F>(
        &self,
        query: SqlQuery,
        map_row: F,
    ) -> Result<Option<T>, SqliteClientError>
    where
        T: Send + 'static,
        F: for<'row> FnMut(&Row<'row>) -> rusqlite::Result<T> + Send + 'static,
    {
        let client = self.clone();
        run_blocking(move || client.read_one_with(&query, map_row)).await
    }

    pub async fn write_async(&self, query: SqlQuery) -> Result<WriteResult, SqliteClientError> {
        let client = self.clone();
        run_blocking(move || client.write(&query)).await
    }

    pub async fn transaction_async<T, F>(&self, operation: F) -> Result<T, SqliteClientError>
    where
        T: Send + 'static,
        F: for<'connection> FnOnce(&SqliteTransaction<'connection>) -> Result<T, SqliteClientError>
            + Send
            + 'static,
    {
        let client = self.clone();
        run_blocking(move || client.transaction(operation)).await
    }
}

fn ensure_read_query(query: &SqlQuery) -> Result<(), SqliteClientError> {
    if !query.kind().is_read() {
        return Err(SqliteClientError::InvalidOperation {
            operation: "read",
            kind: query.kind(),
        });
    }
    Ok(())
}

fn ensure_write_query(query: &SqlQuery) -> Result<(), SqliteClientError> {
    if query.kind().is_read() {
        return Err(SqliteClientError::InvalidOperation {
            operation: "write",
            kind: query.kind(),
        });
    }
    Ok(())
}

fn sql_row_from_row(row: &Row<'_>) -> rusqlite::Result<SqlRow> {
    let statement = row.as_ref();
    let mut values = HashMap::with_capacity(statement.column_count());
    for index in 0..statement.column_count() {
        let column = statement.column_name(index)?.to_string();
        let value = match row.get_ref(index)? {
            ValueRef::Null => SqlValue::Null,
            ValueRef::Integer(value) => SqlValue::Integer(value),
            ValueRef::Real(value) => SqlValue::Real(value),
            ValueRef::Text(value) => SqlValue::Text(String::from_utf8_lossy(value).into_owned()),
            ValueRef::Blob(value) => SqlValue::Blob(value.to_vec()),
        };
        values.insert(column, value);
    }
    Ok(SqlRow { values })
}

async fn run_blocking<T>(
    operation: impl FnOnce() -> Result<T, SqliteClientError> + Send + 'static,
) -> Result<T, SqliteClientError>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| SqliteClientError::BlockingWorker(error.to_string()))?
}

#[cfg(test)]
#[path = "../../../tests/core/clients/sqlite_tests.rs"]
mod tests;
