//! Validate revision transport values before forwarding protected mutations.
pub(super) fn value(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .ok()
        .filter(|n| (1..=9_007_199_254_740_991).contains(n))
        .filter(|_| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| "invalid expectedRevision".into())
}
pub(super) fn query(path: &str) -> Result<i64, String> {
    let (_, query) = path.split_once('?').ok_or("expectedRevision is required")?;
    value(
        query
            .strip_prefix("expectedRevision=")
            .ok_or("invalid expectedRevision")?,
    )
}
pub(super) fn json(body: &serde_json::Value) -> Result<(), String> {
    body.get("expectedRevision")
        .and_then(serde_json::Value::as_i64)
        .filter(|n| (1..=9_007_199_254_740_991).contains(n))
        .map(|_| ())
        .ok_or_else(|| "invalid expectedRevision".into())
}

#[cfg(test)]
#[path = "../../tests/quarry_api/revisions_tests.rs"]
mod tests;
