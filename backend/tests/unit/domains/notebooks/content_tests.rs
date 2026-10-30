use super::*;
use crate::domains::notebooks::test_support::blocks;
#[test]
fn notebook_installed_editor_fixture_is_valid_and_roundtrips_losslessly() {
    let native = blocks();
    let before = native.clone();
    validate(&native).unwrap();
    assert_eq!(before, native);
}
#[test]
fn notebook_rejects_duplicate_media_unsafe_links_unknown_props_and_bad_tables() {
    for mutation in 0..7 {
        let mut native = blocks();
        match mutation {
            0 => native[1]["id"] = native[0]["id"].clone(),
            1 => native[0]["type"] = "image".into(),
            2 => native[0]["content"][1]["href"] = "javascript:alert(1)".into(),
            3 => native[0]["content"][1]["href"] = "https://user:password@example.com".into(),
            4 => native[0]["props"]["extra"] = true.into(),
            5 => native[7]["content"]["rows"][0]["cells"] = serde_json::json!([]),
            _ => native[0]["content"][0]["styles"]["script"] = true.into(),
        }
        assert!(validate(&native).is_err(), "mutation {mutation}");
    }
}
#[test]
fn notebook_content_enforces_depth_nodes_and_bytes() {
    let mut child = blocks()[0].clone();
    child["children"] = serde_json::json!([]);
    for index in 0..33 {
        let mut parent = child.clone();
        parent["id"] = format!("depth-{index}").into();
        parent["children"] = serde_json::json!([child]);
        child = parent;
    }
    assert!(validate(&serde_json::json!([child])).is_err());
    let mut native = blocks();
    native[0]["content"][0]["text"] = "x".repeat(1024 * 1024).into();
    assert_eq!(validate(&native), Err("note content exceeds size limit"));
    assert!(validate(&serde_json::json!({})).is_err());
}
