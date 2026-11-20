use sha2::{Digest, Sha256};
use tiktoken_rs::o200k_base;

pub fn get_token_count(input: impl AsRef<str>) -> usize {
    let bpe = o200k_base().unwrap();
    let tokens = bpe.encode_with_special_tokens(input.as_ref());
    tokens.len()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Deal-scoped content address of an uploaded document.
///
/// `sha256(utf8(deal_id) || 0x00 || utf8(content_sha256))` as lowercase hex. It is the
/// document's `file_id`, its single immutable `version_id` and its manifest `document_id`,
/// so identical bytes in one deal always share one identity and changed bytes never do.
/// PostgreSQL recomputes the same derivation in schema checks.
pub fn document_id_from_content(deal_id: &str, content_sha256: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(deal_id.as_bytes());
    digest.update([0_u8]);
    digest.update(content_sha256.as_bytes());
    format!("{:x}", digest.finalize())
}

/// Whether `value` has the shape of a SHA-256 content address (64 lowercase hex digits).
pub fn is_content_address(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// New deals use canonical UUIDs; existing DEAL- identifiers remain addressable.
pub fn is_deal_id(value: &str) -> bool {
    (value.starts_with("DEAL-")
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_')))
        || uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}

pub fn require_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} is required"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/shared/ids_tests.rs"]
mod tests;
