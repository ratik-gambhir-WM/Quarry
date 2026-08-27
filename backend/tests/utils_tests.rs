use super::*;

#[test]
fn document_id_changes_with_content_or_user() {
    let content_hash = sha256_hex(b"original");
    let changed_hash = sha256_hex(b"changed");

    let original = document_id_from_content("user-1", &content_hash);

    assert_eq!(original.len(), 64);
    assert_ne!(original, document_id_from_content("user-1", &changed_hash));
    assert_ne!(original, document_id_from_content("user-2", &content_hash));
}

#[test]
fn file_version_identity_changes_with_logical_file_or_content() {
    let content_hash = sha256_hex(b"version bytes");
    let original = file_version_id("file-1", &content_hash);

    assert_eq!(original, file_version_id("file-1", &content_hash));
    assert_ne!(original, file_version_id("file-2", &content_hash));
    assert_ne!(original, file_version_id("file-1", &sha256_hex(b"changed")));
    assert_eq!(original.len(), 64);
}

#[test]
fn required_values_reject_empty_and_whitespace_only_input() {
    assert!(require_non_empty("DEAL-1", "dealId").is_ok());
    assert_eq!(
        require_non_empty("  ", "dealId").unwrap_err(),
        "dealId is required"
    );
}
