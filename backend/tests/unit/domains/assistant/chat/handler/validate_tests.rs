use super::*;

#[test]
fn scalar_validation_preserves_text_but_normalizes_model() {
    assert_eq!(
        validate_prompt("  prompt  ".to_string()).unwrap(),
        "  prompt  "
    );
    assert_eq!(
        validate_model(Some("  gpt-5.5  ".to_string()))
            .unwrap()
            .as_deref(),
        Some("gpt-5.5")
    );
    assert!(validate_model(Some(" \n ".to_string())).is_err());
}

#[test]
fn file_references_validate_identity_scope_duplicates_and_combined_limit() {
    let refs = r#"[{"dealId":"DEAL-A","fileId":"file-1","name":"Architecture.pdf"}]"#;
    let parsed = parse_file_references(Some(refs), Some("DEAL-A"), 19).unwrap();
    assert_eq!(parsed[0].file_id, "file-1");
    assert!(parse_file_references(Some(refs), Some("DEAL-B"), 0).is_err());
    assert!(parse_file_references(Some(refs), None, 20).is_err());
    assert!(parse_file_references(Some("not json"), None, 0).is_err());
    assert!(parse_file_references(None, None, 0).unwrap().is_empty());
    let duplicate = r#"[{"dealId":"DEAL-A","fileId":"file-1","name":"A.pdf"},{"dealId":"DEAL-A","fileId":"file-1","name":"B.pdf"}]"#;
    assert!(parse_file_references(Some(duplicate), None, 0).is_err());
    let malicious = r#"[{"dealId":"DEAL-A","fileId":"../secret","name":"A.pdf"}]"#;
    assert!(parse_file_references(Some(malicious), None, 0).is_err());
}

#[test]
fn reference_validation_rejects_bad_names_scope_and_overflow_without_loading_bytes() {
    assert!(parse_file_references(None, Some("../scope"), 0).is_err());
    let valid = r#"[{"dealId":"DEAL-A","fileId":"file-1","name":"Document.pdf"}]"#;
    assert!(parse_file_references(Some(valid), None, usize::MAX).is_err());
    for name in [
        "../document.pdf",
        "bad\\name.pdf",
        " space.pdf",
        "control\n.pdf",
    ] {
        let references =
            serde_json::json!([{"dealId":"DEAL-A","fileId":"file-1","name":name}]).to_string();
        assert!(parse_file_references(Some(&references), Some("DEAL-A"), 0).is_err());
    }
}
