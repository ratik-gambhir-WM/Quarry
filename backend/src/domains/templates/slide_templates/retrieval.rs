//! Classification contracts, bounded provider inputs, and app-scoped slide retrieval.

mod input;
mod metadata;
mod query;
#[cfg(test)]
#[path = "../../../../tests/unit/domains/templates/slide_templates/retrieval_tests.rs"]
mod tests;
mod vector;

pub use input::*;
pub use metadata::*;
pub(crate) use metadata::{js_trim, js_whitespace};
pub use query::*;
pub use vector::*;

use super::{StoredTemplate, StoredTemplatePreview, TemplateKind};
use crate::adapters::postgres::vector::CosineVector;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlideProcessingStage {
    Classification,
    Embedding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetrySlideProcessingOutcome {
    NotFound,
    NotFailed,
    Queued(SlideProcessingStage),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrySlideProcessingResponse {
    pub template_id: String,
    pub stage: SlideProcessingStage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingSlideClassification {
    pub input_fingerprint: String,
    pub prompt_version: String,
    pub schema_version: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSlideClassification {
    pub attempt_count: usize,
    pub classified_at: Option<String>,
    pub classified_fingerprint: Option<String>,
    pub embedding_attempt_count: usize,
    pub embedding_dimensions: Option<usize>,
    pub capability_embedding_document: Option<String>,
    pub capability_embedding_fingerprint: Option<String>,
    pub capability_vector: Option<Vec<f64>>,
    pub embedding_last_error_code: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_next_attempt_at: Option<String>,
    pub embedding_profile_id: Option<String>,
    pub embedding_status: String,
    pub input_fingerprint: String,
    pub last_error_code: Option<String>,
    pub metadata: Option<SlideRetrievalMetadata>,
    pub model: Option<String>,
    pub next_attempt_at: Option<String>,
    pub prompt_version: String,
    pub schema_version: u32,
    pub status: String,
    pub subject_embedding_document: Option<String>,
    pub subject_embedding_fingerprint: Option<String>,
    pub subject_vector: Option<Vec<f64>>,
    pub template_id: String,
}

/// Server-selected template embedding identity. Equal dimensions alone never prove
/// compatibility: provider, model, metric and preparation version are all part of the ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateEmbeddingProfile {
    pub model: String,
    pub dimensions: usize,
}

impl TemplateEmbeddingProfile {
    pub const PROVIDER: &'static str = "openai";
    pub const METRIC: &'static str = "cosine";
    pub const PREPARATION_VERSION: i64 = 1;

    pub fn new(model: impl Into<String>, dimensions: usize) -> Self {
        Self {
            model: model.into(),
            dimensions,
        }
    }

    /// Stable hex identity of the complete profile tuple.
    pub fn id(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for part in [
            Self::PROVIDER,
            self.model.as_str(),
            &self.dimensions.to_string(),
            Self::METRIC,
            &Self::PREPARATION_VERSION.to_string(),
        ] {
            hasher.update(part.as_bytes());
            hasher.update([0]);
        }
        format!("{:x}", hasher.finalize())
    }
}

/// One durable stage attempt. Completion and failure must present this exact token, so an
/// expired attempt cannot publish or fail a reclaimed successor with identical input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageClaim {
    pub template_id: String,
    pub token: String,
}

#[derive(Clone, Debug)]
pub struct SlideClassificationJob {
    pub attempt_count: usize,
    pub claim: StageClaim,
    pub input_fingerprint: String,
    pub kind: TemplateKind,
    pub preview: Option<StoredTemplatePreview>,
    pub template: StoredTemplate,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideEmbeddingJob {
    pub attempt_count: usize,
    pub claim: StageClaim,
    pub capability_document: String,
    pub capability_fingerprint: String,
    pub subject_document: String,
    pub subject_fingerprint: String,
    pub template_id: String,
}

/// A validated subject/capability pair prepared outside SQL and published in one transaction.
/// Construction fails before any mutation when either role is invalid for the profile.
#[derive(Clone, Debug)]
pub struct PreparedSlideEmbeddings {
    pub profile: TemplateEmbeddingProfile,
    pub subject_fingerprint: String,
    pub subject: CosineVector,
    pub capability_fingerprint: String,
    pub capability: CosineVector,
}

impl PreparedSlideEmbeddings {
    /// # Errors
    /// Rejects mismatched dimensions, non-finite values, zero norms and empty fingerprints.
    pub fn new(
        profile: TemplateEmbeddingProfile,
        subject_fingerprint: String,
        subject: &[f64],
        capability_fingerprint: String,
        capability: &[f64],
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !subject_fingerprint.is_empty() && !capability_fingerprint.is_empty(),
            "Embedding fingerprints are required."
        );
        let subject = CosineVector::from_f64(subject, profile.dimensions)?;
        let capability = CosineVector::from_f64(capability, profile.dimensions)?;
        Ok(Self {
            profile,
            subject_fingerprint,
            subject,
            capability_fingerprint,
            capability,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSlideClassification {
    pub classified_at: String,
    pub classified_fingerprint: String,
    pub capability_embedding_document: String,
    pub capability_embedding_fingerprint: String,
    pub metadata: SlideRetrievalMetadata,
    pub model: String,
    pub subject_embedding_document: String,
    pub subject_embedding_fingerprint: String,
    pub template_id: String,
}

/// Vector role. Subject and capability are ranked independently, never against each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbeddingRole {
    Subject,
    Capability,
}

impl EmbeddingRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Subject => "subject",
            Self::Capability => "capability",
        }
    }
}

/// One SQL-ranked identity with its cosine similarity (1 - pgvector cosine distance).
#[derive(Clone, Debug, PartialEq)]
pub struct RankedTemplate {
    pub template_id: String,
    pub similarity: f64,
}

/// A ready stored pair, loaded only for an authorized `similar` source.
#[derive(Clone, Debug)]
pub struct StoredEmbeddingPair {
    pub profile_id: String,
    pub subject: CosineVector,
    pub capability: CosineVector,
}

/// Metadata-only retrieval record. Vectors stay in PostgreSQL and are ranked there.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalClassificationRecord {
    pub classification: StoredSlideClassification,
    pub created_at: String,
    pub kind: TemplateKind,
    pub preview_available: bool,
    pub template_id: String,
    pub title: String,
}
