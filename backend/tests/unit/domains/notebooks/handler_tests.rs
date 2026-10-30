use super::*;
use crate::domains::notebooks::test_support;
#[test]
fn notebook_boundary_validation_rejects_empty_null_revision_only_and_unpaired_content() {
    for body in [
        r#"{"operationId":"00000000-0000-4000-8000-000000000001","expectedRevision":1}"#,
        r#"{"operationId":"00000000-0000-4000-8000-000000000001","expectedRevision":1,"title":null}"#,
        r#"{"operationId":"00000000-0000-4000-8000-000000000001","expectedRevision":1,"blocks":[]}"#,
    ] {
        assert!(validate_save(serde_json::from_str(body).unwrap(), true).is_err());
    }
    assert!(validate_save(test_support::input(""), false).is_ok());
    assert!(label("界".repeat(300), 300, false).is_ok());
    assert!(label("界".repeat(301), 300, false).is_err());
    assert!(label("a\nb".into(), 300, false).is_err());
    assert!(uuid("invalid").is_err());
}
#[test]
fn notebook_query_validation_is_strict_and_preserves_literal_search() {
    for query in [
        "limit=0",
        "limit=101",
        "offset=-1",
        "offset=100001",
        "limit=1&limit=2",
        "unknown=1",
        "q=%",
        "q=+",
    ] {
        assert!(list_options(Some(query.into()), "note").is_err(), "{query}");
    }
    let options = list_options(
        Some("folderId=none&q=%25%5F%5C&limit=1&offset=2".into()),
        "note",
    )
    .unwrap();
    assert_eq!(options.query.as_deref(), Some("%_\\"));
    assert_eq!(options.offset, 2);
    assert!(list_options(Some("dealId=DEAL-A".into()), "folder").is_err());
}
