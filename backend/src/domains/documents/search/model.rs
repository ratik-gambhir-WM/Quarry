use serde::{Deserialize, Serialize};

use crate::domains::documents::graph::query::FileChunkResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HybridSearch {
    pub query_text: String,
    pub limit: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchSource {
    Keyword,
    Semantic,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HybridHit {
    pub chunk_id: String,
    pub deal_id: String,
    pub file_id: String,
    pub version_id: String,
    pub text: String,
    pub chunk_index: i64,
    pub page_start: Option<i64>,
    pub page_end: Option<i64>,
    pub section_path: String,
    pub match_sources: Vec<MatchSource>,
}

impl HybridHit {
    pub(super) fn from_chunk(c: FileChunkResult, match_sources: Vec<MatchSource>) -> Self {
        Self {
            chunk_id: c.chunk_id,
            deal_id: c.deal_id,
            file_id: c.file_id,
            version_id: c.version_id,
            text: c.text,
            chunk_index: c.chunk_index,
            page_start: c.page_start,
            page_end: c.page_end,
            section_path: c.section_path,
            match_sources,
        }
    }
}

#[derive(Serialize)]
pub struct HybridResponse {
    pub results: Vec<HybridHit>,
}
