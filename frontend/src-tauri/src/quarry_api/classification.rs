//! Narrow relay validation for explicit document (re)classification requests.
use serde_json::Value;

const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// `/api/v1/deals/{dealId}/documents/{fileId}/classification`, where the file ID is a
/// deal-scoped content address (64 lowercase hex digits).
pub(super) fn is_classification_path(path: &str) -> bool {
    path.split('?')
        .next()
        .is_some_and(|resource| resource.ends_with("/classification"))
}

pub(super) fn validate(path: &str, body: &Value) -> Result<(), String> {
    let segments = path.split('/').collect::<Vec<_>>();
    let valid_path = !path.contains('?')
        && segments.len() == 8
        && segments[1..4] == ["api", "v1", "deals"]
        && super::service::validate_identifier("dealId", segments[4]).is_ok()
        && segments[4].len() <= 64
        && segments[5] == "documents"
        && segments[6].len() == 64
        && segments[6]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && segments[7] == "classification";
    if !valid_path {
        return Err("Quarry classification path is not allowed".into());
    }
    let object = body
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or("classification body must contain only expectedGeneration")?;
    object
        .get("expectedGeneration")
        .and_then(Value::as_i64)
        .filter(|generation| (0..MAX_SAFE_INTEGER).contains(generation))
        .map(|_| ())
        .ok_or_else(|| "invalid expectedGeneration".into())
}

#[cfg(test)]
#[path = "../../tests/quarry_api/classification_tests.rs"]
mod tests;
