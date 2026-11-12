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
        let mut position = 0;
        let mut quoted = false;
        let mut literal = false;
        let sql = sql
            .chars()
            .map(|c| {
                match c {
                    '"' if !literal => quoted = !quoted,
                    '\'' if !quoted => literal = !literal,
                    _ => {}
                }
                if c == '?' && !quoted && !literal {
                    position += 1;
                    format!("${position}")
                } else {
                    c.to_string()
                }
            })
            .collect();
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
