//! Parameterized SQLite query construction.
//!
//! Values are always emitted as bound parameters. Table and column names are
//! validated and quoted before they become part of a query.

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
