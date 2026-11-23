//! Public classification projection shared by document listings and reclassification.
use super::model::{ClassificationResult, DocumentClassification, RunStatus};

/// Decode persisted enrichment without failing the document listing for malformed JSON.
pub(crate) fn from_stored(
    version_id: String,
    run_status: Option<String>,
    generation: Option<i64>,
    last_error_code: Option<String>,
    result_json: Option<&str>,
    completed_at: Option<i64>,
) -> DocumentClassification {
    let Some(run_status) = run_status else {
        return DocumentClassification::not_requested(version_id);
    };
    let result = result_json.and_then(|json| {
        let parsed = serde_json::from_str::<ClassificationResult>(json)
            .ok()
            .filter(ClassificationResult::is_consistent);
        if parsed.is_none() {
            tracing::warn!(category = "invalid_data", "stored classification ignored");
        }
        parsed
    });
    project(
        version_id,
        run_status,
        generation,
        last_error_code,
        result,
        completed_at,
    )
}

pub(crate) fn completed(
    version_id: String,
    generation: i64,
    result: ClassificationResult,
    completed_at: Option<i64>,
) -> DocumentClassification {
    project(
        version_id,
        RunStatus::Completed.as_str().into(),
        Some(generation),
        None,
        Some(result),
        completed_at,
    )
}

fn project(
    version_id: String,
    run_status: String,
    generation: Option<i64>,
    last_error_code: Option<String>,
    result: Option<ClassificationResult>,
    completed_at: Option<i64>,
) -> DocumentClassification {
    DocumentClassification {
        version_id,
        run_status,
        generation,
        last_error_code,
        result: result.map(|mut result| {
            result.classified_at = completed_at
                .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
                .map(|at| at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
            result
        }),
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/classification/view_tests.rs"]
mod tests;
