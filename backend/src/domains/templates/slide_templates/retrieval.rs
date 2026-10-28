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
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug)]
pub struct SlideClassificationJob {
    pub attempt_count: usize,
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
    pub capability_document: String,
    pub capability_fingerprint: String,
    pub subject_document: String,
    pub subject_fingerprint: String,
    pub template_id: String,
}

/// A completed pair of embedding artifacts published by the background worker.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSlideEmbedding {
    pub capability_fingerprint: String,
    pub capability_vector: Vec<f64>,
    pub dimensions: usize,
    pub model: String,
    pub subject_fingerprint: String,
    pub subject_vector: Vec<f64>,
    pub template_id: String,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteSlideProcessing {
    #[serde(flatten)]
    pub classification: CompleteSlideClassification,
    pub embedding_dimensions: usize,
    pub embedding_model: String,
    pub capability_vector: Vec<f64>,
    pub subject_vector: Vec<f64>,
}

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
