//! Validated cosine vectors using the official pgvector SQLx codec.
//!
//! Extension objects live outside product schemas. Type resolution is qualified
//! so neither a caller nor an isolated schema can shadow the wire type.
use sqlx::{postgres::PgTypeInfo, Postgres, Type};
use thiserror::Error;

/// The maximum full-precision storage dimension supported by pgvector 0.8.6.
pub const MAX_VECTOR_DIMENSIONS: usize = 16_000;

/// Invalid provider output or query vectors. Errors never contain vector values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VectorError {
    #[error("vector dimensions must be between 1 and 16000")]
    Dimensions,
    #[error("vector does not match the selected embedding profile dimensions")]
    ProfileDimensions,
    #[error("vector values and norm must be finite and nonzero after conversion")]
    InvalidValues,
}

/// A finite, nonzero full-precision cosine vector with checked dimensions.
/// This validates numeric representation; the domain must also prove profile identity.
#[derive(Clone, PartialEq)]
pub struct CosineVector(pgvector::Vector);

impl std::fmt::Debug for CosineVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CosineVector")
            .field("dimensions", &self.as_slice().len())
            .finish_non_exhaustive()
    }
}

impl CosineVector {
    /// Validate an f32 vector against its server-selected profile dimension.
    ///
    /// # Errors
    /// Rejects unsupported/mismatched dimensions, nonfinite values and zero norm.
    pub fn new(values: Vec<f32>, dimensions: usize) -> Result<Self, VectorError> {
        check_dimensions(values.len(), dimensions)?;
        check_norm(values.iter().map(|value| f64::from(*value)))?;
        // pgvector 0.8.6 accumulates cosine squared norms in f32, unlike
        // vector_norm(), which accumulates in f64. Guard that representation too.
        let stored_squared_norm = values.iter().map(|value| value * value).sum::<f32>();
        if !stored_squared_norm.is_finite() || stored_squared_norm <= 0.0 {
            return Err(VectorError::InvalidValues);
        }
        Ok(Self(values.into()))
    }

    /// Check provider values before narrowing, then validate their stored f32 form.
    ///
    /// # Errors
    /// Also rejects overflow and vectors whose norm becomes zero on conversion.
    pub fn from_f64(values: &[f64], dimensions: usize) -> Result<Self, VectorError> {
        check_dimensions(values.len(), dimensions)?;
        check_norm(values.iter().copied())?;
        if values.iter().any(|value| value.abs() > f64::from(f32::MAX)) {
            return Err(VectorError::InvalidValues);
        }
        // Rust has no checked float TryFrom. Range/finiteness were checked above;
        // new() checks again after rounding and possible subnormal underflow.
        Self::new(
            values.iter().map(|value| *value as f32).collect(),
            dimensions,
        )
    }

    /// Borrow the checked storage representation.
    pub fn as_slice(&self) -> &[f32] {
        self.0.as_slice()
    }
}

fn check_dimensions(actual: usize, expected: usize) -> Result<(), VectorError> {
    if !(1..=MAX_VECTOR_DIMENSIONS).contains(&expected) {
        return Err(VectorError::Dimensions);
    }
    if actual != expected {
        return Err(VectorError::ProfileDimensions);
    }
    Ok(())
}

fn check_norm(values: impl Iterator<Item = f64>) -> Result<(), VectorError> {
    let mut squared_norm = 0.0;
    for value in values {
        if !value.is_finite() {
            return Err(VectorError::InvalidValues);
        }
        squared_norm += value * value;
    }
    if !squared_norm.is_finite() || squared_norm <= 0.0 {
        return Err(VectorError::InvalidValues);
    }
    Ok(())
}

impl Type<Postgres> for CosineVector {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("quarry_extensions.vector")
    }

    fn compatible(ty: &PgTypeInfo) -> bool {
        // SQLx may cache the qualified parameter name for the returned type OID.
        *ty == Self::type_info() || <pgvector::Vector as Type<Postgres>>::compatible(ty)
    }
}

impl<'q> sqlx::Encode<'q, Postgres> for CosineVector {
    fn encode_by_ref(
        &self,
        buffer: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        self.0.encode_by_ref(buffer)
    }
}

impl<'r> sqlx::Decode<'r, Postgres> for CosineVector {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let decoded = <pgvector::Vector as sqlx::Decode<Postgres>>::decode(value)?;
        let dimensions = decoded.as_slice().len();
        Ok(Self::new(decoded.into(), dimensions)?)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/adapters/postgres/vector_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/integration/adapters/postgres/vector_tests.rs"]
mod integration_tests;
