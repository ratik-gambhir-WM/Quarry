//! Typed classification results, durable run states and the public read model.
use serde::{Deserialize, Serialize};

use super::taxonomy::{Category, Workstream};

/// The only verified Decisions model; pinned per row, never a free-form setting.
pub const MODEL: &str = "gpt-6-luna";
pub const PROMPT_VERSION: &str = "document-classification-v1";
pub const SAMPLE_VERSION: &str = "sample-v1";
/// Bump whenever thresholds or normalization rules change.
pub const POLICY_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Classified,
    NeedsReview,
    InsufficientInformation,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecondaryCategory {
    pub workstream: Workstream,
    pub category: Category,
    pub probability: f64,
}

/// Validated, versioned result. Status decides which nullable fields are legal; see
/// [`ClassificationResult::is_consistent`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationResult {
    pub status: ResultStatus,
    pub primary_workstream: Option<Workstream>,
    pub primary_category: Option<Category>,
    pub secondary_categories: Vec<SecondaryCategory>,
    pub confidence: Option<f64>,
    pub primary_probability: Option<f64>,
    pub suggested_primary_category: Option<Category>,
    pub suggested_primary_workstream: Option<Workstream>,
    pub secondary_assessment_incomplete: bool,
    /// Provider-reported model; null for a local abstention without a provider call.
    pub model: Option<String>,
    pub taxonomy_version: i64,
    pub prompt_version: String,
    pub sample_version: String,
    pub policy_version: i64,
    pub input_fingerprint: String,
    /// Filled from the row's database completion time on read; never stored in JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classified_at: Option<String>,
    pub sampled: bool,
}

impl ClassificationResult {
    pub fn is_consistent(&self) -> bool {
        let accepted = self.status == ResultStatus::Classified;
        let hierarchy = match (self.primary_category, self.primary_workstream) {
            (Some(category), Some(workstream)) => category.workstream() == workstream,
            (None, None) => true,
            _ => false,
        };
        let suggestion = match (
            self.suggested_primary_category,
            self.suggested_primary_workstream,
        ) {
            (Some(category), Some(workstream)) => category.workstream() == workstream,
            (None, None) => true,
            _ => false,
        };
        hierarchy
            && suggestion
            && accepted == self.primary_category.is_some()
            && (accepted || self.secondary_categories.is_empty())
            && (self.status == ResultStatus::NeedsReview
                || self.suggested_primary_category.is_none())
            && self
                .secondary_categories
                .iter()
                .all(|s| s.category.workstream() == s.workstream && unit(s.probability))
            && self.confidence.is_none_or(unit)
            && self.primary_probability.is_none_or(unit)
    }
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// Stored state of a document's classification. Rows are written with the document, so
/// there is no queued or in-flight state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Completed,
    Failed,
    Disabled,
}

impl RunStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Disabled => "disabled",
        }
    }
}

/// Sanitized, bounded failure codes; never raw upstream text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    Timeout,
    Transport,
    RateLimited,
    UpstreamUnavailable,
    Unauthorized,
    RequestRejected,
    InvalidResponse,
    Unavailable,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Transport => "transport",
            Self::RateLimited => "rate_limited",
            Self::UpstreamUnavailable => "upstream_unavailable",
            Self::Unauthorized => "unauthorized",
            Self::RequestRejected => "request_rejected",
            Self::InvalidResponse => "invalid_response",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Public metadata attached to a listed document. Missing rows read as `not_requested`
/// (documents saved before classification existed).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentClassification {
    pub version_id: String,
    pub run_status: String,
    pub generation: Option<i64>,
    pub last_error_code: Option<String>,
    pub result: Option<ClassificationResult>,
}

impl DocumentClassification {
    pub fn not_requested(version_id: String) -> Self {
        Self {
            version_id,
            run_status: "not_requested".into(),
            generation: None,
            last_error_code: None,
            result: None,
        }
    }
}
