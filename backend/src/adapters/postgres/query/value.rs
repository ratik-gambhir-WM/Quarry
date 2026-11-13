#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<bool> for SqlValue {
    fn from(value: bool) -> Self {
        Self::Integer(i64::from(value))
    }
}

macro_rules! integer_value {
    ($($value_type:ty),+ $(,)?) => {
        $(
            impl From<$value_type> for SqlValue {
                fn from(value: $value_type) -> Self {
                    Self::Integer(i64::from(value))
                }
            }
        )+
    };
}

integer_value!(i8, i16, i32, i64, u8, u16, u32);

impl From<f32> for SqlValue {
    fn from(value: f32) -> Self {
        Self::Real(f64::from(value))
    }
}

impl From<f64> for SqlValue {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl From<String> for SqlValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for SqlValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<u8>> for SqlValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Blob(value)
    }
}

impl From<&[u8]> for SqlValue {
    fn from(value: &[u8]) -> Self {
        Self::Blob(value.to_vec())
    }
}

impl<T> From<Option<T>> for SqlValue
where
    T: Into<Self>,
{
    fn from(value: Option<T>) -> Self {
        value.map(Into::into).unwrap_or(Self::Null)
    }
}

impl sqlx::Type<sqlx::Postgres> for SqlValue {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_oid(sqlx::postgres::types::Oid(0))
    }
    fn compatible(_: &sqlx::postgres::PgTypeInfo) -> bool {
        true
    }
}
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for SqlValue {
    fn produces(&self) -> Option<sqlx::postgres::PgTypeInfo> {
        use sqlx::Type;
        match self {
            Self::Null => None,
            Self::Integer(_) => Some(<i64 as Type<sqlx::Postgres>>::type_info()),
            Self::Real(_) => Some(<f64 as Type<sqlx::Postgres>>::type_info()),
            Self::Text(_) => Some(<String as Type<sqlx::Postgres>>::type_info()),
            Self::Blob(_) => Some(<Vec<u8> as Type<sqlx::Postgres>>::type_info()),
        }
    }
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        match self {
            Self::Null => Ok(sqlx::encode::IsNull::Yes),
            Self::Integer(v) => v.encode_by_ref(buf),
            Self::Real(v) => v.encode_by_ref(buf),
            Self::Text(v) => v.encode_by_ref(buf),
            Self::Blob(v) => v.encode_by_ref(buf),
        }
    }
}

impl<T: Clone + Into<SqlValue>> From<&T> for SqlValue {
    fn from(v: &T) -> Self {
        v.clone().into()
    }
}

impl<const N: usize> From<[u8; N]> for SqlValue {
    fn from(v: [u8; N]) -> Self {
        Self::Blob(v.to_vec())
    }
}
