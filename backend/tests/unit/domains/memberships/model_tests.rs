use super::*;
#[test]
fn roles_have_stable_wire_codes() {
    for role in [
        DealRole::Owner,
        DealRole::Manager,
        DealRole::Contributor,
        DealRole::Viewer,
    ] {
        assert_eq!(
            serde_json::to_string(&role).unwrap(),
            format!("\"{}\"", role.code())
        );
        assert_eq!(
            serde_json::from_str::<DealRole>(&format!("\"{}\"", role.code())).unwrap(),
            role
        );
    }
    for value in ["admin", "OWNER", "", "analyst"] {
        assert!(serde_json::from_value::<DealRole>(serde_json::json!(value)).is_err());
    }
}
