use super::*;
use serde_json::json;

const FILE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn accepts_only_the_exact_classification_route_and_bounded_body() {
    let path = format!("/api/v1/deals/DEAL-1/documents/{FILE}/classification");
    assert!(is_classification_path(&path));
    assert!(validate(&path, &json!({"expectedGeneration": 0})).is_ok());
    assert!(validate(&path, &json!({"expectedGeneration": 4})).is_ok());
    for body in [
        json!({}),
        json!({"expectedGeneration": -1}),
        json!({"expectedGeneration": 1.5}),
        json!({"expectedGeneration": "1"}),
        json!({"expectedGeneration": 9_007_199_254_740_991_i64}),
        json!({"expectedGeneration": 1, "model": "other"}),
        json!([1]),
    ] {
        assert!(validate(&path, &body).is_err(), "{body}");
    }
    for invalid in [
        "/api/v1/deals/DEAL-1/documents/file-1/classification".to_string(),
        format!(
            "/api/v1/deals/DEAL-1/documents/{}/classification",
            FILE.to_uppercase()
        ),
        format!("/api/v1/deals/DEAL 1/documents/{FILE}/classification"),
        format!("/api/v1/deals/DEAL-1/documents/{FILE}/classification?x=1"),
        format!("/api/v1/deals/DEAL-1/files/{FILE}/classification"),
    ] {
        assert!(
            validate(&invalid, &json!({"expectedGeneration": 0})).is_err(),
            "{invalid}"
        );
    }
}
