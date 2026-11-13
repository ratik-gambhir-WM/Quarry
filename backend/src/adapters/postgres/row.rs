//! Owned values and checked row decoding at the PostgreSQL boundary.
use super::{client::PostgresError, query::SqlValue};
use sqlx::{Column, Row as _, TypeInfo, ValueRef};
use std::collections::HashMap;

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

pub struct Row<'r>(pub(super) &'r sqlx::postgres::PgRow);
impl Row<'_> {
    pub fn get<I: RowIndex, T: FromSqlValue>(&self, index: I) -> Result<T, PostgresError> {
        let index = index.resolve(self.0)?;
        T::decode(value(self.0, index)?)
    }
    pub(super) fn owned(&self) -> Result<SqlRow, PostgresError> {
        Ok(SqlRow {
            values: self
                .0
                .columns()
                .iter()
                .enumerate()
                .map(|(i, c)| Ok((c.name().to_owned(), value(self.0, i)?)))
                .collect::<Result<_, PostgresError>>()?,
        })
    }
}
pub trait RowIndex {
    fn resolve(self, row: &sqlx::postgres::PgRow) -> Result<usize, PostgresError>;
}
impl RowIndex for usize {
    fn resolve(self, row: &sqlx::postgres::PgRow) -> Result<usize, PostgresError> {
        if self < row.columns().len() {
            Ok(self)
        } else {
            Err(PostgresError::decode("column index out of bounds"))
        }
    }
}
impl RowIndex for &str {
    fn resolve(self, row: &sqlx::postgres::PgRow) -> Result<usize, PostgresError> {
        row.columns()
            .iter()
            .position(|c| c.name() == self)
            .ok_or_else(|| PostgresError::decode("unknown column"))
    }
}
fn value(row: &sqlx::postgres::PgRow, i: usize) -> Result<SqlValue, PostgresError> {
    if row.try_get_raw(i)?.is_null() {
        return Ok(SqlValue::Null);
    }
    Ok(match row.column(i).type_info().name() {
        "INT8" => SqlValue::Integer(row.try_get(i)?),
        "INT4" => SqlValue::Integer(i64::from(row.try_get::<i32, _>(i)?)),
        "INT2" => SqlValue::Integer(i64::from(row.try_get::<i16, _>(i)?)),
        "FLOAT8" => SqlValue::Real(row.try_get(i)?),
        "FLOAT4" => SqlValue::Real(f64::from(row.try_get::<f32, _>(i)?)),
        "BOOL" => SqlValue::Integer(i64::from(row.try_get::<bool, _>(i)?)),
        "BYTEA" => SqlValue::Blob(row.try_get(i)?),
        "TEXT" | "VARCHAR" | "NAME" | "CHAR" => SqlValue::Text(row.try_get(i)?),
        _ => return Err(PostgresError::decode("unsupported PostgreSQL column type")),
    })
}
pub trait FromSqlValue: Sized {
    fn decode(value: SqlValue) -> Result<Self, PostgresError>;
}
impl FromSqlValue for String {
    fn decode(v: SqlValue) -> Result<Self, PostgresError> {
        if let SqlValue::Text(v) = v {
            Ok(v)
        } else {
            Err(PostgresError::decode("expected text"))
        }
    }
}
impl FromSqlValue for Vec<u8> {
    fn decode(v: SqlValue) -> Result<Self, PostgresError> {
        if let SqlValue::Blob(v) = v {
            Ok(v)
        } else {
            Err(PostgresError::decode("expected bytes"))
        }
    }
}
impl FromSqlValue for f64 {
    fn decode(v: SqlValue) -> Result<Self, PostgresError> {
        match v {
            SqlValue::Real(v) => Ok(v),
            _ => Err(PostgresError::decode("expected real")),
        }
    }
}
macro_rules! integer {
    ($($t:ty),+) => {
        $(
            impl FromSqlValue for $t {
                fn decode(value: SqlValue) -> Result<Self, PostgresError> {
                    if let SqlValue::Integer(value) = value {
                        Self::try_from(value).map_err(PostgresError::decode)
                    } else {
                        Err(PostgresError::decode("expected integer"))
                    }
                }
            }
        )+
    };
}
integer!(i64, i32, i16, i8, u64, u32, u16, u8, usize);
impl FromSqlValue for bool {
    fn decode(v: SqlValue) -> Result<Self, PostgresError> {
        match v {
            SqlValue::Integer(0) => Ok(false),
            SqlValue::Integer(1) => Ok(true),
            _ => Err(PostgresError::decode("expected boolean integer")),
        }
    }
}
impl<T: FromSqlValue> FromSqlValue for Option<T> {
    fn decode(v: SqlValue) -> Result<Self, PostgresError> {
        match v {
            SqlValue::Null => Ok(None),
            v => T::decode(v).map(Some),
        }
    }
}
