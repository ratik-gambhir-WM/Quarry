use super::*;
use crate::domains::documents::classification::{
    model::ResultStatus,
    policy::{self, Provenance},
    taxonomy::Category,
};

fn result() -> ClassificationResult {
    policy::insufficient(&Provenance {
        fingerprint: "a".repeat(64),
        sampled: true,
    })
}

#[test]
fn stored_and_fresh_results_have_identical_public_metadata() {
    for status in [
        ResultStatus::Classified,
        ResultStatus::NeedsReview,
        ResultStatus::InsufficientInformation,
        ResultStatus::Refused,
    ] {
        let mut result = result();
        result.status = status;
        if status == ResultStatus::Classified {
            result.primary_category = Some(Category::Sdlc);
            result.primary_workstream = Some(Category::Sdlc.workstream());
        }
        if status == ResultStatus::NeedsReview {
            result.suggested_primary_category = Some(Category::Sdlc);
            result.suggested_primary_workstream = Some(Category::Sdlc.workstream());
        }
        let json = serde_json::to_string(&result).unwrap();
        for timestamp in [None, Some(0), Some(1_790_000_000), Some(i64::MAX)] {
            let stored = from_stored(
                "version".into(),
                Some("completed".into()),
                Some(2),
                None,
                Some(&json),
                timestamp,
            );
            let fresh = completed("version".into(), 2, result.clone(), timestamp);
            assert_eq!(stored, fresh, "{status:?} at {timestamp:?}");
            assert_eq!(
                serde_json::to_value(stored).unwrap(),
                serde_json::to_value(fresh).unwrap()
            );
        }
    }
}

#[test]
fn missing_rows_and_failed_or_disabled_runs_keep_the_existing_wire_shape() {
    assert_eq!(
        serde_json::to_value(from_stored("v".into(), None, None, None, None, None)).unwrap(),
        serde_json::json!({
            "versionId": "v", "runStatus": "not_requested", "generation": null,
            "lastErrorCode": null, "result": null,
        })
    );
    for (status, error) in [("disabled", None), ("failed", Some("timeout"))] {
        let view = from_stored(
            "v".into(),
            Some(status.into()),
            Some(1),
            error.map(str::to_string),
            None,
            None,
        );
        assert_eq!(view.run_status, status);
        assert_eq!(view.last_error_code.as_deref(), error);
        assert_eq!(view.result, None);
    }
}

#[test]
fn malformed_or_inconsistent_stored_results_are_omitted_without_losing_metadata() {
    let mut inconsistent = result();
    inconsistent.primary_category = Some(Category::Sdlc);
    let invalid_hierarchy = serde_json::to_string(&inconsistent).unwrap();
    for json in ["{", "null", &invalid_hierarchy] {
        let view = from_stored(
            "v".into(),
            Some("completed".into()),
            Some(4),
            None,
            Some(json),
            Some(0),
        );
        assert_eq!(view.version_id, "v");
        assert_eq!(view.run_status, "completed");
        assert_eq!(view.generation, Some(4));
        assert_eq!(view.result, None);
    }
}

#[test]
fn database_completion_time_replaces_any_timestamp_in_stored_json() {
    let mut result = result();
    result.classified_at = Some("untrusted stored timestamp".into());
    let json = serde_json::to_string(&result).unwrap();
    let view = from_stored(
        "v".into(),
        Some("completed".into()),
        Some(1),
        None,
        Some(&json),
        Some(0),
    );
    assert_eq!(
        view.result.unwrap().classified_at.as_deref(),
        Some("1970-01-01T00:00:00Z")
    );
}
