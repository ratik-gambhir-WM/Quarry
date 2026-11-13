//! Parameterized PostgreSQL query construction.
//!
//! Values are always emitted as bound parameters. Table and column names are
//! validated and quoted before they become part of a query.
//! Each builder's `build()` converts validation failures to `PostgresError`
//! without executing SQL, so its result can be passed directly to the client.

mod builder;
pub(crate) mod lexical;
mod statement;
mod unicode61;
mod value;

pub use builder::{
    ComparisonOperator, Condition, ConflictUpdate, DeleteBuilder, InsertBuilder, JoinClause,
    JoinCondition, JoinType, LockWait, RowLock, SelectBuilder, SortDirection, SqlBuilder,
    SqlBuilderError, UpdateBuilder,
};
pub use statement::{QueryKind, SqlQuery};
pub use value::SqlValue;

#[cfg(test)]
#[path = "../../../../tests/unit/adapters/postgres/query/tests.rs"]
mod tests;
