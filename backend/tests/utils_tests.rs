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
