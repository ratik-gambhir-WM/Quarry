use super::*;
#[test]
fn notebook_patch_is_bounded_and_scoped_to_canonical_resource_paths() {
    let id = "00000000-0000-4000-8000-000000000001";
    let root = format!("/api/v1/notebooks/{id}");
    for path in [
        root.clone(),
        format!("{root}/notes/{id}"),
        format!("{root}/folders/{id}"),
    ] {
        validate_patch(&path, &serde_json::json!({"title":"a"})).unwrap();
    }
    for path in [
        "https://example.com/api/v1/notebooks".into(),
        format!("{root}?x=1"),
        format!("{root}/notes/../deals"),
        format!("{root}/save-jobs/{id}"),
        format!("{root}/notes/%2e%2e"),
        format!("{root}/notes/{id}/extra"),
    ] {
        assert!(
            validate_patch(&path, &serde_json::json!({})).is_err(),
            "{path}"
        );
    }
    assert!(validate_patch(
        &root,
        &serde_json::json!({"title":"x".repeat(MAX_NOTEBOOK_JSON)})
    )
    .is_err());
    assert!(validate_patch(&root, &Value::Null).is_err());
}
#[test]
fn notebook_relay_preserves_split_utf8_crlf_and_rejects_oversized_events() {
    let text = "event: completed\r\ndata: {\"title\":\"café\"}\r\n\r\n";
    let mut parser = NotebookRelayParser::default();
    let mut events = Vec::new();
    for byte in text.as_bytes() {
        events.extend(parser.push(&[*byte]).unwrap());
    }
    assert_eq!(
        events,
        vec![("completed".into(), "{\"title\":\"café\"}".into())]
    );
    assert!(parser.push(&vec![b'x'; 65537]).is_err());
}
