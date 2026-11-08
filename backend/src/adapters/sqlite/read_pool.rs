//! Fixed-size read-only pool. Checkout waits only on a blocking worker, never a Tokio worker.
use std::{
    path::Path,
    sync::{Condvar, Mutex},
};

use rusqlite::{Connection, OpenFlags};

use super::client::{SqliteClientError, DEFAULT_BUSY_TIMEOUT};

pub(super) const READ_CONNECTIONS: usize = 4;

pub(super) struct ReadPool {
    connections: Vec<Mutex<Connection>>,
    available: Mutex<Vec<usize>>,
    returned: Condvar,
}

impl ReadPool {
    pub(super) fn open(path: &Path) -> Result<Self, SqliteClientError> {
        let connections = (0..READ_CONNECTIONS)
            .map(|_| {
                let connection = Connection::open_with_flags(
                    path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                connection.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
                connection.pragma_update(None, "foreign_keys", "ON")?;
                connection.pragma_update(None, "query_only", "ON")?;
                Ok(Mutex::new(connection))
            })
            .collect::<Result<Vec<_>, SqliteClientError>>()?;
        Ok(Self {
            connections,
            available: Mutex::new((0..READ_CONNECTIONS).collect()),
            returned: Condvar::new(),
        })
    }

    pub(super) fn with_connection<T, E>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<SqliteClientError>,
    {
        let lease = self.checkout().map_err(E::from)?;
        let mut connection = self.connections[lease.index]
            .lock()
            .map_err(|_| E::from(SqliteClientError::ConnectionLockPoisoned))?;
        operation(&mut connection)
    }

    fn checkout(&self) -> Result<ReadLease<'_>, SqliteClientError> {
        let mut available = self
            .available
            .lock()
            .map_err(|_| SqliteClientError::ConnectionLockPoisoned)?;
        loop {
            if let Some(index) = available.pop() {
                return Ok(ReadLease { pool: self, index });
            }
            available = self
                .returned
                .wait(available)
                .map_err(|_| SqliteClientError::ConnectionLockPoisoned)?;
        }
    }
}

struct ReadLease<'pool> {
    pool: &'pool ReadPool,
    index: usize,
}

impl Drop for ReadLease<'_> {
    fn drop(&mut self) {
        // Always return the slot, including during unwinding or query/mapper failures.
        let mut available = self
            .pool
            .available
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        available.push(self.index);
        self.pool.returned.notify_one();
    }
}
