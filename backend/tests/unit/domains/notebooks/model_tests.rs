use super::*;
#[test]
fn notebook_patch_preserves_omission_null_and_value() {
    let missing: NotebookInput = serde_json::from_str("{}").unwrap();
    assert!(missing.deal_id.missing());
    let null: NotebookInput = serde_json::from_str(r#"{"dealId":null}"#).unwrap();
    assert_eq!(null.deal_id, PatchField::Null);
    let value: NotebookInput = serde_json::from_str(r#"{"dealId":"DEAL-A"}"#).unwrap();
    assert_eq!(value.deal_id, PatchField::Value("DEAL-A".into()));
    assert!(serde_json::from_str::<NotebookInput>(r#"{"title":"a","userId":2}"#).is_err());
    assert!(serde_json::from_str::<NotebookInput>(r#"{"title":"a","title":"b"}"#).is_err());
}
#[test]
fn notebook_revision_is_js_safe_and_blocks_are_native_json() {
    let mut input = crate::domains::notebooks::test_support::input("");
    input.expected_revision = PatchField::Value(MAX_REVISION);
    let value = serde_json::to_value(input).unwrap();
    assert_eq!(value["expectedRevision"], MAX_REVISION);
    assert!(value["blocks"].is_array());
}
