use std::collections::HashSet;

use thiserror::Error;

use super::{QueryKind, SqlQuery, SqlValue};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SqlBuilderError {
    #[error("SQL identifier cannot be empty")]
    EmptyIdentifier,
    #[error("SQL identifier contains a null byte: `{0}`")]
    InvalidIdentifier(String),
    #[error("column `{0}` was specified more than once")]
    DuplicateColumn(String),
    #[error("an INSERT query needs at least one value")]
    MissingInsertValues,
    #[error("an UPDATE query needs at least one assignment")]
    MissingUpdateAssignments,
    #[error("ON CONFLICT DO UPDATE needs at least one conflict target column")]
    MissingConflictTarget,
    #[error("ON CONFLICT DO UPDATE needs at least one assignment")]
    MissingConflictAssignments,
    #[error("{0} requires a condition; call allow_all_rows() to explicitly target every row")]
    MissingCondition(&'static str),
    #[error("an IN condition for `{0}` needs at least one value")]
    EmptyInValues(String),
    #[error("a {join_type} JOIN on `{table}` needs at least one ON condition")]
    MissingJoinCondition {
        join_type: &'static str,
        table: String,
    },
    #[error("a CROSS JOIN on `{0}` cannot have an ON condition")]
    UnexpectedCrossJoinCondition(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Like,
    NotLike,
}

impl ComparisonOperator {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
            Self::Like => "LIKE",
            Self::NotLike => "NOT LIKE",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    FullOuter,
    Cross,
}

impl JoinType {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Inner => "INNER",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
            Self::FullOuter => "FULL OUTER",
            Self::Cross => "CROSS",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum JoinPredicate {
    Columns {
        left: String,
        operator: ComparisonOperator,
        right: String,
    },
    Value {
        column: String,
        operator: ComparisonOperator,
        value: SqlValue,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct JoinCondition(JoinPredicate);

impl JoinCondition {
    pub fn columns(
        left: impl Into<String>,
        operator: ComparisonOperator,
        right: impl Into<String>,
    ) -> Self {
        Self(JoinPredicate::Columns {
            left: left.into(),
            operator,
            right: right.into(),
        })
    }

    pub fn equal(left: impl Into<String>, right: impl Into<String>) -> Self {
        Self::columns(left, ComparisonOperator::Equal, right)
    }

    pub fn value(
        column: impl Into<String>,
        operator: ComparisonOperator,
        value: impl Into<SqlValue>,
    ) -> Self {
        Self(JoinPredicate::Value {
            column: column.into(),
            operator,
            value: value.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Predicate {
    Comparison {
        column: String,
        operator: ComparisonOperator,
        value: SqlValue,
    },
    In {
        column: String,
        values: Vec<SqlValue>,
        negated: bool,
    },
    Null {
        column: String,
        negated: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition(Predicate);

impl Condition {
    pub fn compare(
        column: impl Into<String>,
        operator: ComparisonOperator,
        value: impl Into<SqlValue>,
    ) -> Self {
        Self(Predicate::Comparison {
            column: column.into(),
            operator,
            value: value.into(),
        })
    }

    pub fn equal(column: impl Into<String>, value: impl Into<SqlValue>) -> Self {
        Self::compare(column, ComparisonOperator::Equal, value)
    }

    pub fn not_equal(column: impl Into<String>, value: impl Into<SqlValue>) -> Self {
        Self::compare(column, ComparisonOperator::NotEqual, value)
    }

    pub fn in_values<I, V>(column: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<SqlValue>,
    {
        Self(Predicate::In {
            column: column.into(),
            values: values.into_iter().map(Into::into).collect(),
            negated: false,
        })
    }

    pub fn not_in_values<I, V>(column: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<SqlValue>,
    {
        Self(Predicate::In {
            column: column.into(),
            values: values.into_iter().map(Into::into).collect(),
            negated: true,
        })
    }

    pub fn is_null(column: impl Into<String>) -> Self {
        Self(Predicate::Null {
            column: column.into(),
            negated: false,
        })
    }

    pub fn is_not_null(column: impl Into<String>) -> Self {
        Self(Predicate::Null {
            column: column.into(),
            negated: true,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConditionConnector {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
struct ConnectedCondition {
    connector: ConditionConnector,
    condition: Condition,
}

#[derive(Debug, Clone, PartialEq)]
struct ConnectedJoinCondition {
    connector: ConditionConnector,
    condition: JoinCondition,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JoinClause {
    join_type: JoinType,
    table: String,
    alias: Option<String>,
    conditions: Vec<ConnectedJoinCondition>,
}

impl JoinClause {
    pub fn new(join_type: JoinType, table: impl Into<String>) -> Self {
        Self {
            join_type,
            table: table.into(),
            alias: None,
            conditions: Vec::new(),
        }
    }

    pub fn inner(table: impl Into<String>) -> Self {
        Self::new(JoinType::Inner, table)
    }

    pub fn left(table: impl Into<String>) -> Self {
        Self::new(JoinType::Left, table)
    }

    pub fn right(table: impl Into<String>) -> Self {
        Self::new(JoinType::Right, table)
    }

    pub fn full_outer(table: impl Into<String>) -> Self {
        Self::new(JoinType::FullOuter, table)
    }

    pub fn cross(table: impl Into<String>) -> Self {
        Self::new(JoinType::Cross, table)
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }

    pub fn on(mut self, condition: JoinCondition) -> Self {
        push_join_condition(&mut self.conditions, ConditionConnector::And, condition);
        self
    }

    pub fn and_on(mut self, condition: JoinCondition) -> Self {
        push_join_condition(&mut self.conditions, ConditionConnector::And, condition);
        self
    }

    pub fn or_on(mut self, condition: JoinCondition) -> Self {
        push_join_condition(&mut self.conditions, ConditionConnector::Or, condition);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    fn as_sql(self) -> &'static str {
        match self {
            Self::Ascending => "ASC",
            Self::Descending => "DESC",
        }
    }
}

pub struct SqlBuilder;

impl SqlBuilder {
    pub fn select(table: impl Into<String>) -> SelectBuilder {
        SelectBuilder::new(table)
    }

    pub fn insert_into(table: impl Into<String>) -> InsertBuilder {
        InsertBuilder::new(table)
    }

    pub fn update(table: impl Into<String>) -> UpdateBuilder {
        UpdateBuilder::new(table)
    }

    pub fn delete_from(table: impl Into<String>) -> DeleteBuilder {
        DeleteBuilder::new(table)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ConflictAssignmentValue {
    Excluded(String),
    Bound(SqlValue),
    CurrentTimestamp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConflictUpdate {
    target_columns: Vec<String>,
    assignments: Vec<(String, ConflictAssignmentValue)>,
}

impl ConflictUpdate {
    pub fn new<I, S>(target_columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            target_columns: target_columns.into_iter().map(Into::into).collect(),
            assignments: Vec::new(),
        }
    }

    /// Updates `column` from the value that SQLite attempted to insert.
    pub fn set_excluded(mut self, column: impl Into<String>) -> Self {
        let column = column.into();
        self.assignments
            .push((column.clone(), ConflictAssignmentValue::Excluded(column)));
        self
    }

    /// Updates `column` from another bound parameter.
    pub fn set(mut self, column: impl Into<String>, value: impl Into<SqlValue>) -> Self {
        self.assignments
            .push((column.into(), ConflictAssignmentValue::Bound(value.into())));
        self
    }

    /// Updates `column` using SQLite's `CURRENT_TIMESTAMP` expression.
    pub fn set_current_timestamp(mut self, column: impl Into<String>) -> Self {
        self.assignments
            .push((column.into(), ConflictAssignmentValue::CurrentTimestamp));
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
enum InsertConflict {
    DoNothing { target_columns: Vec<String> },
    DoUpdate(ConflictUpdate),
}

#[derive(Debug, Clone)]
pub struct SelectBuilder {
    table: String,
    table_alias: Option<String>,
    columns: Vec<String>,
    distinct: bool,
    joins: Vec<JoinClause>,
    conditions: Vec<ConnectedCondition>,
    order_by: Vec<(String, SortDirection)>,
    limit: Option<usize>,
    offset: Option<usize>,
}

impl SelectBuilder {
    fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            table_alias: None,
            columns: Vec::new(),
            distinct: false,
            joins: Vec::new(),
            conditions: Vec::new(),
            order_by: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    pub fn columns<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.columns.extend(columns.into_iter().map(Into::into));
        self
    }

    pub fn column(mut self, column: impl Into<String>) -> Self {
        self.columns.push(column.into());
        self
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.table_alias = Some(alias.into());
        self
    }

    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    pub fn join(mut self, join: JoinClause) -> Self {
        self.joins.push(join);
        self
    }

    pub fn inner_join(
        self,
        table: impl Into<String>,
        left_column: impl Into<String>,
        right_column: impl Into<String>,
    ) -> Self {
        self.join(JoinClause::inner(table).on(JoinCondition::equal(left_column, right_column)))
    }

    pub fn left_join(
        self,
        table: impl Into<String>,
        left_column: impl Into<String>,
        right_column: impl Into<String>,
    ) -> Self {
        self.join(JoinClause::left(table).on(JoinCondition::equal(left_column, right_column)))
    }

    pub fn right_join(
        self,
        table: impl Into<String>,
        left_column: impl Into<String>,
        right_column: impl Into<String>,
    ) -> Self {
        self.join(JoinClause::right(table).on(JoinCondition::equal(left_column, right_column)))
    }

    pub fn full_outer_join(
        self,
        table: impl Into<String>,
        left_column: impl Into<String>,
        right_column: impl Into<String>,
    ) -> Self {
        self.join(JoinClause::full_outer(table).on(JoinCondition::equal(left_column, right_column)))
    }

    pub fn cross_join(self, table: impl Into<String>) -> Self {
        self.join(JoinClause::cross(table))
    }

    pub fn and_where(mut self, condition: Condition) -> Self {
        push_condition(&mut self.conditions, ConditionConnector::And, condition);
        self
    }

    pub fn or_where(mut self, condition: Condition) -> Self {
        push_condition(&mut self.conditions, ConditionConnector::Or, condition);
        self
    }

    pub fn order_by(mut self, column: impl Into<String>, direction: SortDirection) -> Self {
        self.order_by.push((column.into(), direction));
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn build(self) -> Result<SqlQuery, SqlBuilderError> {
        let table = quote_table(&self.table, self.table_alias.as_deref())?;
        ensure_unique_columns(&self.columns)?;
        let columns = if self.columns.is_empty() {
            "*".to_string()
        } else {
            self.columns
                .iter()
                .map(|column| quote_select_column(column))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        };

        let distinct = if self.distinct { "DISTINCT " } else { "" };
        let mut sql = format!("SELECT {distinct}{columns} FROM {table}");
        let mut parameters = Vec::new();
        append_joins(&mut sql, &self.joins, &mut parameters)?;
        append_conditions(&mut sql, &self.conditions, &mut parameters)?;

        if !self.order_by.is_empty() {
            let order_by = self
                .order_by
                .iter()
                .map(|(column, direction)| {
                    Ok(format!(
                        "{} {}",
                        quote_identifier(column)?,
                        direction.as_sql()
                    ))
                })
                .collect::<Result<Vec<_>, SqlBuilderError>>()?
                .join(", ");
            sql.push_str(" ORDER BY ");
            sql.push_str(&order_by);
        }

        match (self.limit, self.offset) {
            (Some(limit), Some(offset)) => {
                sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));
            }
            (Some(limit), None) => sql.push_str(&format!(" LIMIT {limit}")),
            (None, Some(offset)) => sql.push_str(&format!(" LIMIT -1 OFFSET {offset}")),
            (None, None) => {}
        }

        Ok(SqlQuery::new(sql, parameters, QueryKind::Select))
    }
}

#[derive(Debug, Clone)]
pub struct InsertBuilder {
    table: String,
    values: Vec<(String, SqlValue)>,
    conflict: Option<InsertConflict>,
}

impl InsertBuilder {
    fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            values: Vec::new(),
            conflict: None,
        }
    }

    pub fn value(mut self, column: impl Into<String>, value: impl Into<SqlValue>) -> Self {
        self.values.push((column.into(), value.into()));
        self
    }

    pub fn on_conflict_do_nothing<I, S>(mut self, target_columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.conflict = Some(InsertConflict::DoNothing {
            target_columns: target_columns.into_iter().map(Into::into).collect(),
        });
        self
    }

    pub fn on_conflict_update(mut self, update: ConflictUpdate) -> Self {
        self.conflict = Some(InsertConflict::DoUpdate(update));
        self
    }

    pub fn build(self) -> Result<SqlQuery, SqlBuilderError> {
        if self.values.is_empty() {
            return Err(SqlBuilderError::MissingInsertValues);
        }

        let table = quote_identifier(&self.table)?;
        let column_names = self
            .values
            .iter()
            .map(|(column, _)| column.clone())
            .collect::<Vec<_>>();
        ensure_unique_columns(&column_names)?;
        let columns = column_names
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let placeholders = vec!["?"; self.values.len()].join(", ");
        let mut parameters = self
            .values
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        let mut sql = format!("INSERT INTO {table} ({columns}) VALUES ({placeholders})");
        append_insert_conflict(&mut sql, self.conflict, &mut parameters)?;

        Ok(SqlQuery::new(sql, parameters, QueryKind::Insert))
    }
}

#[derive(Debug, Clone)]
pub struct UpdateBuilder {
    table: String,
    assignments: Vec<(String, SqlValue)>,
    conditions: Vec<ConnectedCondition>,
    allow_all_rows: bool,
}

impl UpdateBuilder {
    fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            assignments: Vec::new(),
            conditions: Vec::new(),
            allow_all_rows: false,
        }
    }

    pub fn set(mut self, column: impl Into<String>, value: impl Into<SqlValue>) -> Self {
        self.assignments.push((column.into(), value.into()));
        self
    }

    pub fn and_where(mut self, condition: Condition) -> Self {
        push_condition(&mut self.conditions, ConditionConnector::And, condition);
        self
    }

    pub fn or_where(mut self, condition: Condition) -> Self {
        push_condition(&mut self.conditions, ConditionConnector::Or, condition);
        self
    }

    pub fn allow_all_rows(mut self) -> Self {
        self.allow_all_rows = true;
        self
    }

    pub fn build(self) -> Result<SqlQuery, SqlBuilderError> {
        if self.assignments.is_empty() {
            return Err(SqlBuilderError::MissingUpdateAssignments);
        }
        require_conditions("UPDATE", &self.conditions, self.allow_all_rows)?;

        let table = quote_identifier(&self.table)?;
        let column_names = self
            .assignments
            .iter()
            .map(|(column, _)| column.clone())
            .collect::<Vec<_>>();
        ensure_unique_columns(&column_names)?;
        let assignments = column_names
            .iter()
            .map(|column| Ok(format!("{} = ?", quote_identifier(column)?)))
            .collect::<Result<Vec<_>, SqlBuilderError>>()?
            .join(", ");
        let mut parameters = self
            .assignments
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        let mut sql = format!("UPDATE {table} SET {assignments}");
        append_conditions(&mut sql, &self.conditions, &mut parameters)?;

        Ok(SqlQuery::new(sql, parameters, QueryKind::Update))
    }
}

#[derive(Debug, Clone)]
pub struct DeleteBuilder {
    table: String,
    conditions: Vec<ConnectedCondition>,
    allow_all_rows: bool,
}

impl DeleteBuilder {
    fn new(table: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            conditions: Vec::new(),
            allow_all_rows: false,
        }
    }

    pub fn and_where(mut self, condition: Condition) -> Self {
        push_condition(&mut self.conditions, ConditionConnector::And, condition);
        self
    }

    pub fn or_where(mut self, condition: Condition) -> Self {
        push_condition(&mut self.conditions, ConditionConnector::Or, condition);
        self
    }

    pub fn allow_all_rows(mut self) -> Self {
        self.allow_all_rows = true;
        self
    }

    pub fn build(self) -> Result<SqlQuery, SqlBuilderError> {
        require_conditions("DELETE", &self.conditions, self.allow_all_rows)?;

        let table = quote_identifier(&self.table)?;
        let mut sql = format!("DELETE FROM {table}");
        let mut parameters = Vec::new();
        append_conditions(&mut sql, &self.conditions, &mut parameters)?;

        Ok(SqlQuery::new(sql, parameters, QueryKind::Delete))
    }
}

fn push_condition(
    conditions: &mut Vec<ConnectedCondition>,
    connector: ConditionConnector,
    condition: Condition,
) {
    conditions.push(ConnectedCondition {
        connector,
        condition,
    });
}

fn append_insert_conflict(
    sql: &mut String,
    conflict: Option<InsertConflict>,
    parameters: &mut Vec<SqlValue>,
) -> Result<(), SqlBuilderError> {
    let Some(conflict) = conflict else {
        return Ok(());
    };

    match conflict {
        InsertConflict::DoNothing { target_columns } => {
            sql.push_str(" ON CONFLICT");
            append_conflict_target(sql, &target_columns)?;
            sql.push_str(" DO NOTHING");
        }
        InsertConflict::DoUpdate(update) => {
            if update.target_columns.is_empty() {
                return Err(SqlBuilderError::MissingConflictTarget);
            }
            if update.assignments.is_empty() {
                return Err(SqlBuilderError::MissingConflictAssignments);
            }

            ensure_unique_columns(&update.target_columns)?;
            let assignment_columns = update
                .assignments
                .iter()
                .map(|(column, _)| column.clone())
                .collect::<Vec<_>>();
            ensure_unique_columns(&assignment_columns)?;

            sql.push_str(" ON CONFLICT");
            append_conflict_target(sql, &update.target_columns)?;
            sql.push_str(" DO UPDATE SET ");

            let mut assignments = Vec::with_capacity(update.assignments.len());
            for (column, value) in update.assignments {
                let column = quote_identifier(&column)?;
                let assignment = match value {
                    ConflictAssignmentValue::Excluded(source_column) => {
                        format!("{column} = excluded.{}", quote_identifier(&source_column)?)
                    }
                    ConflictAssignmentValue::Bound(value) => {
                        parameters.push(value);
                        format!("{column} = ?")
                    }
                    ConflictAssignmentValue::CurrentTimestamp => {
                        format!("{column} = CURRENT_TIMESTAMP")
                    }
                };
                assignments.push(assignment);
            }
            sql.push_str(&assignments.join(", "));
        }
    }

    Ok(())
}

fn append_conflict_target(
    sql: &mut String,
    target_columns: &[String],
) -> Result<(), SqlBuilderError> {
    if target_columns.is_empty() {
        return Ok(());
    }
    ensure_unique_columns(target_columns)?;
    let target = target_columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    sql.push_str(" (");
    sql.push_str(&target);
    sql.push(')');
    Ok(())
}

fn push_join_condition(
    conditions: &mut Vec<ConnectedJoinCondition>,
    connector: ConditionConnector,
    condition: JoinCondition,
) {
    conditions.push(ConnectedJoinCondition {
        connector,
        condition,
    });
}

fn append_joins(
    sql: &mut String,
    joins: &[JoinClause],
    parameters: &mut Vec<SqlValue>,
) -> Result<(), SqlBuilderError> {
    for join in joins {
        if join.join_type == JoinType::Cross && !join.conditions.is_empty() {
            return Err(SqlBuilderError::UnexpectedCrossJoinCondition(
                join.table.clone(),
            ));
        }
        if join.join_type != JoinType::Cross && join.conditions.is_empty() {
            return Err(SqlBuilderError::MissingJoinCondition {
                join_type: join.join_type.as_sql(),
                table: join.table.clone(),
            });
        }

        sql.push(' ');
        sql.push_str(join.join_type.as_sql());
        sql.push_str(" JOIN ");
        sql.push_str(&quote_table(&join.table, join.alias.as_deref())?);

        if join.join_type == JoinType::Cross {
            continue;
        }

        sql.push_str(" ON ");
        for (index, connected) in join.conditions.iter().enumerate() {
            if index > 0 {
                sql.push_str(match connected.connector {
                    ConditionConnector::And => " AND ",
                    ConditionConnector::Or => " OR ",
                });
            }

            sql.push('(');
            match &connected.condition.0 {
                JoinPredicate::Columns {
                    left,
                    operator,
                    right,
                } => {
                    sql.push_str(&quote_identifier(left)?);
                    sql.push(' ');
                    sql.push_str(operator.as_sql());
                    sql.push(' ');
                    sql.push_str(&quote_identifier(right)?);
                }
                JoinPredicate::Value {
                    column,
                    operator,
                    value,
                } => {
                    sql.push_str(&quote_identifier(column)?);
                    match (operator, value) {
                        (ComparisonOperator::Equal, SqlValue::Null) => sql.push_str(" IS NULL"),
                        (ComparisonOperator::NotEqual, SqlValue::Null) => {
                            sql.push_str(" IS NOT NULL")
                        }
                        _ => {
                            sql.push(' ');
                            sql.push_str(operator.as_sql());
                            sql.push_str(" ?");
                            parameters.push(value.clone());
                        }
                    }
                }
            }
            sql.push(')');
        }
    }

    Ok(())
}

fn append_conditions(
    sql: &mut String,
    conditions: &[ConnectedCondition],
    parameters: &mut Vec<SqlValue>,
) -> Result<(), SqlBuilderError> {
    if conditions.is_empty() {
        return Ok(());
    }

    sql.push_str(" WHERE ");
    for (index, connected) in conditions.iter().enumerate() {
        if index > 0 {
            sql.push_str(match connected.connector {
                ConditionConnector::And => " AND ",
                ConditionConnector::Or => " OR ",
            });
        }

        sql.push('(');
        match &connected.condition.0 {
            Predicate::Comparison {
                column,
                operator,
                value,
            } => {
                sql.push_str(&quote_identifier(column)?);
                match (operator, value) {
                    (ComparisonOperator::Equal, SqlValue::Null) => sql.push_str(" IS NULL"),
                    (ComparisonOperator::NotEqual, SqlValue::Null) => sql.push_str(" IS NOT NULL"),
                    _ => {
                        sql.push(' ');
                        sql.push_str(operator.as_sql());
                        sql.push_str(" ?");
                        parameters.push(value.clone());
                    }
                }
            }
            Predicate::In {
                column,
                values,
                negated,
            } => {
                if values.is_empty() {
                    return Err(SqlBuilderError::EmptyInValues(column.clone()));
                }
                sql.push_str(&quote_identifier(column)?);
                sql.push_str(if *negated { " NOT IN (" } else { " IN (" });
                sql.push_str(&vec!["?"; values.len()].join(", "));
                sql.push(')');
                parameters.extend(values.iter().cloned());
            }
            Predicate::Null { column, negated } => {
                sql.push_str(&quote_identifier(column)?);
                sql.push_str(if *negated { " IS NOT NULL" } else { " IS NULL" });
            }
        }
        sql.push(')');
    }

    Ok(())
}

fn require_conditions(
    operation: &'static str,
    conditions: &[ConnectedCondition],
    allow_all_rows: bool,
) -> Result<(), SqlBuilderError> {
    if conditions.is_empty() && !allow_all_rows {
        return Err(SqlBuilderError::MissingCondition(operation));
    }
    Ok(())
}

fn ensure_unique_columns(columns: &[String]) -> Result<(), SqlBuilderError> {
    let mut seen = HashSet::with_capacity(columns.len());
    for column in columns {
        if !seen.insert(column) {
            return Err(SqlBuilderError::DuplicateColumn(column.clone()));
        }
    }
    Ok(())
}

fn quote_identifier(identifier: &str) -> Result<String, SqlBuilderError> {
    if identifier.is_empty() || identifier.split('.').any(str::is_empty) {
        return Err(SqlBuilderError::EmptyIdentifier);
    }
    if identifier.contains('\0') {
        return Err(SqlBuilderError::InvalidIdentifier(identifier.to_string()));
    }

    Ok(identifier
        .split('.')
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join("."))
}

fn quote_select_column(column: &str) -> Result<String, SqlBuilderError> {
    if column == "*" {
        return Ok("*".to_string());
    }
    if let Some(table) = column.strip_suffix(".*") {
        return Ok(format!("{}.*", quote_identifier(table)?));
    }
    quote_identifier(column)
}

fn quote_table(table: &str, alias: Option<&str>) -> Result<String, SqlBuilderError> {
    let table = quote_identifier(table)?;
    match alias {
        Some(alias) => Ok(format!("{table} AS {}", quote_identifier(alias)?)),
        None => Ok(table),
    }
}
