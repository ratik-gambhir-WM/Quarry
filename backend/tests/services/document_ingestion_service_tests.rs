use super::*;

#[test]
fn upload_content_hash_is_stable_until_document_bytes_change() {
    let original = UploadedDocument {
        filename: "memo.pdf".to_string(),
        bytes: b"same bytes".to_vec(),
    };
    let renamed = UploadedDocument {
        filename: "renamed.pdf".to_string(),
        bytes: b"same bytes".to_vec(),
    };
    let changed = UploadedDocument {
        filename: "memo.pdf".to_string(),
        bytes: b"changed bytes".to_vec(),
    };

    let original_hash = uploaded_document_content_hash(&original).unwrap();
    assert_eq!(
        original_hash,
        uploaded_document_content_hash(&renamed).unwrap()
    );
    assert_ne!(
        original_hash,
        uploaded_document_content_hash(&changed).unwrap()
    );
}
