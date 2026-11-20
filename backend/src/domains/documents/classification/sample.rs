//! `sample-v1`: bounded, deterministic evidence sampling over parser-owned chunks.
//!
//! Sampling is two-phase so ingestion (all text in memory) and recovery (text loaded
//! from PostgreSQL) produce byte-identical evidence: [`plan`] selects token windows from
//! chunk metadata alone, then [`render`] cuts those windows from only the chosen texts.
//! No raw bytes, vectors or full document copies are made.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use super::model::SAMPLE_VERSION;
use crate::shared::ids::sha256_hex;

/// Application token estimate (o200k), not a claim of exact provider tokenization.
pub const MAX_SAMPLE_TOKENS: u64 = 6_000;
pub const MAX_SAMPLE_BYTES: usize = 32 * 1024;
const LABEL_TOKEN_ALLOWANCE: u64 = 400;
const HEAD_TOKENS: u64 = 2_000;
const MAX_WINDOWS: u64 = 8;
const MAX_CONTEXT_BYTES: usize = 200;
const MAX_PROVENANCE_ENTRIES: usize = 200;
const SHRINK_ATTEMPTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceKind {
    ExtractedText,
    ImageDescription,
}

impl EvidenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExtractedText => "extracted_text",
            Self::ImageDescription => "image_description",
        }
    }
}

/// Weak, untrusted context. Filenames never determine a category.
#[derive(Clone, Copy, Debug)]
pub struct SampleContext<'a> {
    pub filename: &'a str,
    pub source_type: &'a str,
    pub evidence: EvidenceKind,
}

/// Borrowed chunk metadata in sequence order.
#[derive(Clone, Copy, Debug)]
pub struct ChunkMeta<'a> {
    pub index: i64,
    pub token_count: u64,
    pub sha256: &'a str,
    pub section: Option<&'a str>,
    pub pages: Option<(i64, i64)>,
}

/// Token window `[start, end)` inside one chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Segment {
    position: usize,
    start: u64,
    end: u64,
}

#[derive(Clone, Debug)]
pub struct SamplePlan {
    sampled: bool,
    total_tokens: u64,
    segments: Vec<Segment>,
}

impl SamplePlan {
    /// Chunk indices whose text [`render`] needs.
    pub fn needed_chunks(&self, meta: &[ChunkMeta<'_>]) -> Vec<i64> {
        let mut needed: Vec<i64> = self
            .segments
            .iter()
            .map(|segment| meta[segment.position].index)
            .collect();
        needed.dedup();
        needed
    }
}

/// Bounded owned evidence, ready to persist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceSample {
    pub text: String,
    pub provenance: String,
    pub hash: String,
    pub sampled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleOutcome {
    Evidence(EvidenceSample),
    /// No usable extracted text: abstain locally without a provider request.
    Empty(EvidenceSample),
}

/// Select windows from metadata only. `budget` is the evidence token budget.
pub fn plan(meta: &[ChunkMeta<'_>], budget: u64) -> SamplePlan {
    let total_tokens: u64 = meta.iter().map(|chunk| chunk.token_count).sum();
    let budget = budget.max(1);
    if total_tokens <= budget {
        let segments = meta
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.token_count > 0)
            .map(|(position, chunk)| Segment {
                position,
                start: 0,
                end: chunk.token_count,
            })
            .collect();
        return SamplePlan {
            sampled: false,
            total_tokens,
            segments,
        };
    }
    // Anchors depend only on the document, never on the budget, so a shrunken plan is
    // always a subset of a larger one (recovery loads texts for the initial plan only).
    let anchor = HEAD_TOKENS.min(total_tokens);
    let head = anchor.min(budget / 3);
    let rest = budget - head;
    let windows = MAX_WINDOWS;
    let width = rest / windows;
    let mut ranges = vec![(0, head)];
    let mut previous_end = anchor;
    for k in 1..=windows {
        // End-anchored, evenly spaced windows; the last one always reaches the end.
        let end = anchor + (total_tokens - anchor) * k / windows;
        let start = end.saturating_sub(width).max(previous_end);
        if end > start {
            ranges.push((start, end));
        }
        previous_end = end;
    }
    SamplePlan {
        sampled: true,
        total_tokens,
        segments: split_into_chunks(meta, &ranges),
    }
}

/// Map global token ranges onto per-chunk segments, merging adjacent ones.
fn split_into_chunks(meta: &[ChunkMeta<'_>], ranges: &[(u64, u64)]) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut offset = 0_u64;
    for (position, chunk) in meta.iter().enumerate() {
        let chunk_start = offset;
        let chunk_end = offset + chunk.token_count;
        offset = chunk_end;
        for &(start, end) in ranges {
            let overlap_start = start.max(chunk_start);
            let overlap_end = end.min(chunk_end);
            if overlap_start >= overlap_end {
                continue;
            }
            let segment = Segment {
                position,
                start: overlap_start - chunk_start,
                end: overlap_end - chunk_start,
            };
            match segments.last_mut() {
                Some(last) if last.position == position && last.end >= segment.start => {
                    last.end = last.end.max(segment.end);
                }
                _ => segments.push(segment),
            }
        }
    }
    segments
}

/// Render planned windows, then verify both ceilings with the tokenizer, shrinking
/// the plan (always to a subset of `texts`) until the evidence fits.
pub fn build(
    context: SampleContext<'_>,
    meta: &[ChunkMeta<'_>],
    texts: &HashMap<i64, &str>,
) -> SampleOutcome {
    let header = header(context);
    let header_tokens = token_count(&header);
    let mut budget = MAX_SAMPLE_TOKENS
        .saturating_sub(LABEL_TOKEN_ALLOWANCE)
        .saturating_sub(header_tokens);
    for _ in 0..SHRINK_ATTEMPTS {
        let plan = plan(meta, budget);
        let (body, selected) = render(&plan, meta, texts);
        if body.trim().is_empty() {
            return SampleOutcome::Empty(finish(context, &plan, String::new(), &selected));
        }
        let text = format!("{header}{body}");
        if text.len() <= MAX_SAMPLE_BYTES && token_count(&text) <= MAX_SAMPLE_TOKENS {
            return SampleOutcome::Evidence(finish(context, &plan, text, &selected));
        }
        budget = budget * 4 / 5;
    }
    let plan = plan(meta, budget);
    let (body, selected) = render(&plan, meta, texts);
    let mut text = format!("{header}{body}");
    truncate_at_char_boundary(&mut text, MAX_SAMPLE_BYTES);
    SampleOutcome::Evidence(finish(context, &plan, text, &selected))
}

/// Initial plan used to decide which stored texts must be loaded for [`build`].
pub fn initial_plan(context: SampleContext<'_>, meta: &[ChunkMeta<'_>]) -> SamplePlan {
    let budget = MAX_SAMPLE_TOKENS
        .saturating_sub(LABEL_TOKEN_ALLOWANCE)
        .saturating_sub(token_count(&header(context)));
    plan(meta, budget)
}

type Selected = Vec<(i64, String, usize, usize)>;

fn render(
    plan: &SamplePlan,
    meta: &[ChunkMeta<'_>],
    texts: &HashMap<i64, &str>,
) -> (String, Selected) {
    let mut body = String::new();
    let mut selected = Vec::with_capacity(plan.segments.len());
    if plan.sampled {
        body.push_str(
            "Excerpts sampled from a longer document; omitted content was not examined.\n\n",
        );
    }
    for segment in &plan.segments {
        let chunk = meta[segment.position];
        let Some(text) = texts.get(&chunk.index).copied() else {
            continue;
        };
        let (start, end) = byte_window(text, chunk.token_count, segment.start, segment.end);
        let excerpt = text[start..end].trim();
        if excerpt.is_empty() {
            continue;
        }
        body.push_str(&label(chunk, plan.sampled, meta.len()));
        body.push_str(excerpt);
        body.push_str("\n\n");
        selected.push((chunk.index, chunk.sha256.to_string(), start, end));
    }
    (body, selected)
}

/// Proportional token-to-byte window at UTF-8 boundaries, preferring whitespace edges.
fn byte_window(text: &str, tokens: u64, start: u64, end: u64) -> (usize, usize) {
    if tokens == 0 || (start == 0 && end >= tokens) {
        return (0, text.len());
    }
    let length = text.len() as u128;
    let to_byte = |token: u64| -> usize {
        usize::try_from(length * u128::from(token) / u128::from(tokens)).unwrap_or(text.len())
    };
    let mut first = floor_boundary(text, to_byte(start));
    let mut last = floor_boundary(text, to_byte(end.min(tokens)));
    if start > 0 {
        if let Some(space) = text[first..last].find(char::is_whitespace) {
            if space < 64 {
                first += space;
            }
        }
    }
    if end < tokens {
        if let Some(space) = text[first..last].rfind(char::is_whitespace) {
            if last - (first + space) < 64 {
                last = first + space;
            }
        }
    }
    (first, last.max(first))
}

fn floor_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn truncate_at_char_boundary(text: &mut String, limit: usize) {
    if text.len() > limit {
        let boundary = floor_boundary(text, limit);
        text.truncate(boundary);
    }
}

fn header(context: SampleContext<'_>) -> String {
    format!(
        "Document filename (weak context only): {}\nSource type: {}\nEvidence kind: {}\n---\n\n",
        sanitize(context.filename),
        sanitize(context.source_type),
        match context.evidence {
            EvidenceKind::ExtractedText => "extracted text",
            EvidenceKind::ImageDescription =>
                "image description (a model interpretation of an image, not extracted text)",
        }
    )
}

fn label(chunk: ChunkMeta<'_>, sampled: bool, total: usize) -> String {
    let mut label = if sampled {
        format!("[excerpt from chunk {} of {total}", chunk.index)
    } else {
        format!("[chunk {} of {total}", chunk.index)
    };
    if let Some((first, last)) = chunk.pages {
        if first == last {
            label.push_str(&format!(" | page {first}"));
        } else {
            label.push_str(&format!(" | pages {first}-{last}"));
        }
    }
    if let Some(section) = chunk.section.filter(|section| !section.trim().is_empty()) {
        label.push_str(" | section ");
        label.push_str(&sanitize(section));
    }
    label.push_str("]\n");
    label
}

/// Single-line, bounded, control-free rendering of untrusted context.
fn sanitize(value: &str) -> String {
    let mut clean: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    truncate_at_char_boundary(&mut clean, MAX_CONTEXT_BYTES);
    clean.trim().to_string()
}

fn finish(
    context: SampleContext<'_>,
    plan: &SamplePlan,
    text: String,
    selected: &Selected,
) -> EvidenceSample {
    let mut digest = Sha256::new();
    for (index, sha, start, end) in selected {
        digest.update(format!("{index}:{sha}:{start}:{end};").as_bytes());
    }
    let entries: Vec<_> = selected
        .iter()
        .take(MAX_PROVENANCE_ENTRIES)
        .map(|(index, sha, start, end)| json!([index, &sha[..sha.len().min(16)], start, end]))
        .collect();
    let provenance = json!({
        "sampleVersion": SAMPLE_VERSION,
        "evidenceKind": context.evidence.as_str(),
        "sampled": plan.sampled,
        "totalTokens": plan.total_tokens,
        "selectedCount": selected.len(),
        "selected": entries,
        "selectedDigest": format!("{:x}", digest.finalize()),
    })
    .to_string();
    EvidenceSample {
        hash: sha256_hex(text.as_bytes()),
        text,
        provenance,
        sampled: plan.sampled,
    }
}

fn token_count(text: &str) -> u64 {
    let tokens = tiktoken_rs::o200k_base_singleton()
        .encode_ordinary(text)
        .len();
    u64::try_from(tokens).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/classification/sample_tests.rs"]
mod tests;
