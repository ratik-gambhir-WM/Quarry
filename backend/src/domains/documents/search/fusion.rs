use super::model::{HybridHit, MatchSource};
use crate::domains::documents::graph::query::FileChunkResult;
use std::collections::{BTreeMap, HashSet};

// Full immutable identity; text from a different generation can never contribute a rank.
pub(super) type Identity = (String, String, String, String, String, String, i64);
pub(super) fn identity(c: &FileChunkResult) -> Identity {
    (
        c.deal_id.clone(),
        c.file_id.clone(),
        c.version_id.clone(),
        c.chunk_id.clone(),
        c.index_generation.clone(),
        c.chunk_sha256.clone(),
        c.chunk_index,
    )
}

fn sort_identity(c: &FileChunkResult) -> (&str, &str, &str, &str, &str, &str, i64) {
    (
        &c.deal_id,
        &c.file_id,
        &c.version_id,
        &c.chunk_id,
        &c.index_generation,
        &c.chunk_sha256,
        c.chunk_index,
    )
}

pub(super) struct RankedHit {
    pub chunk: FileChunkResult,
    pub sources: Vec<MatchSource>,
    score: f64,
    best_rank: usize,
}

pub(super) fn fuse(
    keyword: Vec<FileChunkResult>,
    semantic: Vec<FileChunkResult>,
) -> Vec<RankedHit> {
    let mut combined = BTreeMap::<Identity, RankedHit>::new();
    for (source, hits) in [
        (MatchSource::Keyword, keyword),
        (MatchSource::Semantic, semantic),
    ] {
        let mut seen = HashSet::new();
        let mut rank = 0;
        for chunk in hits {
            let id = identity(&chunk);
            if !seen.insert(id.clone()) {
                continue;
            }
            rank += 1;
            let entry = combined.entry(id).or_insert(RankedHit {
                chunk,
                sources: Vec::with_capacity(2),
                score: 0.0,
                best_rank: rank,
            });
            entry.score += 1.0 / (60.0 + rank as f64);
            entry.best_rank = entry.best_rank.min(rank);
            entry.sources.push(source);
        }
    }
    combined.into_values().collect()
}

pub(super) fn top(mut hits: Vec<RankedHit>, limit: usize) -> Vec<HybridHit> {
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.best_rank.cmp(&b.best_rank))
            .then_with(|| sort_identity(&a.chunk).cmp(&sort_identity(&b.chunk)))
    });
    hits.truncate(limit);
    hits.into_iter()
        .map(|h| HybridHit::from_chunk(h.chunk, h.sources))
        .collect()
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/search/fusion_tests.rs"]
mod tests;
