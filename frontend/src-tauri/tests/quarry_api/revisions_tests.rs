use super::*;
#[test]
fn revision_transport_rejects_missing_duplicated_fractional_and_unsafe_values() {
    assert_eq!(query("/api/v1/deals/DEAL-1?expectedRevision=7").unwrap(), 7);
    for path in [
        "/api/v1/deals/DEAL-1",
        "/api/v1/deals/DEAL-1?expectedRevision=0",
        "/api/v1/deals/DEAL-1?expectedRevision=1&expectedRevision=2",
        "/api/v1/deals/DEAL-1?expectedRevision=1.5",
        "/api/v1/deals/DEAL-1?expectedRevision=9007199254740992",
    ] {
        assert!(query(path).is_err());
    }
    for value in [
        serde_json::json!({}),
        serde_json::json!({"expectedRevision":null}),
        serde_json::json!({"expectedRevision":1.5}),
        serde_json::json!({"expectedRevision":9007199254740992_i64}),
    ] {
        assert!(json(&value).is_err());
    }
    assert!(json(&serde_json::json!({"expectedRevision":9_007_199_254_740_991_i64})).is_ok());
}
