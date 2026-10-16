//! Parameterized SQLite query construction.
//!
//! Values are always emitted as bound parameters. Table and column names are
//! validated and quoted before they become part of a query.
//! Each builder's `build()` converts validation failures to `SqliteClientError`
//! without executing SQL, so its result can be passed directly to the client.

mod builder;
mod statement;
mod value;

pub use builder::{
    ComparisonOperator, Condition, ConflictUpdate, DeleteBuilder, InsertBuilder, JoinClause,
    JoinCondition, JoinType, SelectBuilder, SortDirection, SqlBuilder, SqlBuilderError,
    UpdateBuilder,
};
pub use statement::{QueryKind, SqlQuery};
pub use value::SqlValue;

#[cfg(test)]
#[path = "../../../../tests/unit/adapters/sqlite/query/tests.rs"]
mod tests;
