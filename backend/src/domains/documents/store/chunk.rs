//! Stable document search wire models.
use serde::{Deserialize, Serialize};

pub const MAX_FILE_CHUNK_SEARCH_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChunkVectorSearch {
    #[serde(default)]
    pub deal_id: String,
    pub query_embedding: Vec<f32>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChunkKeywordSearch {
    #[serde(default)]
    pub deal_id: String,
    pub query_text: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChunkResult {
    pub chunk_id: String,
    pub deal_id: String,
    pub file_id: String,
    pub version_id: String,
    pub index_generation: String,
    pub chunk_index: i64,
    pub text: String,
    pub chunk_sha256: String,
    pub token_count: i64,
    pub page_start: Option<i64>,
    pub page_end: Option<i64>,
    pub char_start: i64,
    pub char_end: i64,
    pub section_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorFileChunkHit {
    #[serde(flatten)]
    pub chunk: FileChunkResult,
    /// Legacy-normalized cosine distance in 0..=1; lower is closer.
    pub distance: f64,
}

/// The pinned Helix runtime reports normalized cosine distance: (1 - cosine) / 2.
/// Keep raw distances on legacy DTOs and convert explicitly before similarity policy.
/// Verified against the pinned image with aligned, scaled, oblique and orthogonal vectors.
pub(crate) fn legacy_cosine_similarity(distance: f64) -> Option<f64> {
    (distance.is_finite() && (0.0..=1.0).contains(&distance)).then_some(1.0 - 2.0 * distance)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordFileChunkHit {
    #[serde(flatten)]
    pub chunk: FileChunkResult,
    pub score: f64,
}
