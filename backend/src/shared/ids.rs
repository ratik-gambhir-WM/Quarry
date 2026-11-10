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

pub fn document_id_from_content(scope_id: &str, content_hash: &str) -> String {
    sha256_hex(format!("{scope_id}\0{content_hash}").as_bytes())
}

pub fn file_version_id(file_id: &str, content_hash: &str) -> String {
    sha256_hex(format!("{file_id}\0{content_hash}").as_bytes())
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
