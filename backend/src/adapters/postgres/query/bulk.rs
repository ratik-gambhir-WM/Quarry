//! Bounded multi-row INSERTs; execution remains on the caller's transaction.
use super::{builder::quote_identifier, QueryKind, SqlBuilderError, SqlQuery, SqlValue};
use crate::adapters::postgres::client::PostgresError;
pub struct BatchInsertBuilder {
    table: String,
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
}
impl BatchInsertBuilder {
    pub(super) fn new(table: String, columns: Vec<String>, rows: Vec<Vec<SqlValue>>) -> Self {
        Self {
            table,
            columns,
            rows,
        }
    }
    /// Produce one bound multi-row INSERT with no conflict-skipping behavior.
    ///
    /// # Errors
    /// Rejects empty/ragged batches, duplicate columns, invalid identifiers, more
    /// than 128 rows or more than 60,000 bound parameters before any SQL executes.
    pub fn build(self) -> Result<SqlQuery, PostgresError> {
        let count = self
            .rows
            .len()
            .checked_mul(self.columns.len())
            .ok_or(SqlBuilderError::InvalidInsertBatch)?;
        if self.columns.is_empty()
            || self.rows.is_empty()
            || self.rows.len() > 128
            || count > 60_000
            || self.rows.iter().any(|row| row.len() != self.columns.len())
        {
            return Err(SqlBuilderError::InvalidInsertBatch.into());
        }
        let mut seen = std::collections::HashSet::new();
        let columns = self
            .columns
            .iter()
            .map(|column| {
                if !seen.insert(column) {
                    return Err(SqlBuilderError::DuplicateColumn(column.clone()));
                }
                quote_identifier(column)
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let row = format!("({})", vec!["?"; self.columns.len()].join(", "));
        let sql = format!(
            "INSERT INTO {} ({columns}) VALUES {}",
            quote_identifier(&self.table)?,
            vec![row; self.rows.len()].join(", ")
        );
        Ok(SqlQuery::new(
            sql,
            self.rows.into_iter().flatten().collect(),
            QueryKind::Insert,
        ))
    }
}
