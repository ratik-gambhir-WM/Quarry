//! Narrow shared-deal PATCH transport. Authorization and product rules stay in Axum.
use serde_json::Value;
pub(super) fn validate_patch(path: &str, body: &Value) -> Result<(), String> {
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() != 7
        || parts[..4] != ["", "api", "v1", "deals"]
        || !(parts[4].starts_with("DEAL-") || super::notebooks::canonical_id(parts[4]).is_ok())
    {
        return Err("deal PATCH path is not allowed".into());
    }
    super::service::validate_identifier("dealId", parts[4])?;
    if !body.is_object()
        || serde_json::to_vec(body)
            .map_err(|_| "invalid deal JSON")?
            .len()
            > 16384
    {
        return Err("invalid or oversized deal JSON".into());
    }
    match parts[5] {
        "members" => {
            if parts[6].parse::<i64>().ok().filter(|n| *n > 0).is_none() {
                return Err("invalid userId".into());
            }
        }
        "teams" => {
            super::notebooks::canonical_id(parts[6])?;
        }
        _ => return Err("deal PATCH path is not allowed".into()),
    }
    Ok(())
}
#[cfg(test)]
#[path = "../../tests/quarry_membership_tests.rs"]
mod tests;
