//! Immutable, document-owned identity models and pure integrity validation.
use crate::shared::error::RepositoryError;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkIdentity {
    pub chunk_id: String,
    pub index_generation: String,
    pub chunk_index: i64,
    pub chunk_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityManifest {
    pub derivation_version: i64,
    pub document_id: String,
    pub active_generation: String,
    pub chunks: Vec<ChunkIdentity>,
}
impl IdentityManifest {
    pub fn validate(&self) -> Result<(), RepositoryError> {
        if ![1, 2].contains(&self.derivation_version)
            || self.document_id.is_empty()
            || self.active_generation.is_empty()
        {
            return Err(RepositoryError::invalid("invalid identity manifest"));
        }
        let mut ids = std::collections::HashSet::new();
        let mut orders = std::collections::HashSet::new();
        for c in &self.chunks {
            if c.chunk_id.is_empty()
                || c.index_generation.is_empty()
                || c.chunk_sha256.len() != 64
                || c.chunk_index < 0
                || !ids.insert(&c.chunk_id)
                || !orders.insert((&c.index_generation, c.chunk_index))
            {
                return Err(RepositoryError::invalid(
                    "ambiguous chunk identity manifest",
                ));
            }
        }
        Ok(())
    }
}
