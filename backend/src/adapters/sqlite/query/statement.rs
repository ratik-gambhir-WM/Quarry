use super::SqlValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKind {
    Select,
    Insert,
    Update,
    Delete,
}

impl QueryKind {
    pub fn is_read(self) -> bool {
        matches!(self, Self::Select)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlQuery {
    sql: String,
    parameters: Vec<SqlValue>,
    kind: QueryKind,
}

impl SqlQuery {
    pub(crate) fn new(sql: String, parameters: Vec<SqlValue>, kind: QueryKind) -> Self {
        Self {
            sql,
            parameters,
            kind,
        }
    }

    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn parameters(&self) -> &[SqlValue] {
        &self.parameters
    }

    pub fn kind(&self) -> QueryKind {
        self.kind
    }
}
