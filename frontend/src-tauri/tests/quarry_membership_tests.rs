use super::*;
#[test]
fn membership_patch_is_narrow_and_bounds_json() {
    let body = serde_json::json!({"role":"viewer","expectedRevision":1});
    assert!(validate_patch("/api/v1/deals/DEAL-1/members/2", &body).is_ok());
    assert!(validate_patch(
        "/api/v1/deals/5346b615-818b-4b25-85da-045507df2db4/members/2",
        &body
    )
    .is_ok());
    assert!(validate_patch("/api/v1/deals/5346b615-818b-4b25-85da-045507df2db4/teams/00000000-0000-4000-8000-000000000001", &body).is_ok());
    assert!(validate_patch(
        "/api/v1/deals/DEAL-1/teams/00000000-0000-4000-8000-000000000001",
        &body
    )
    .is_ok());
    for path in [
        "/api/deals/DEAL-1/members/2",
        "/api/v1/deals/not-a-uuid/members/2",
        "/api/v1/deals/5346B615-818B-4B25-85DA-045507DF2DB4/members/2",
        "/api/v1/deals/DEAL-1/members/0",
        "/api/v1/deals/DEAL-1/members/2?x=y",
        "/api/v1/deals/DEAL-1/teams/not-a-uuid",
        "/api/v1/deals/DEAL-1/update",
        "/api/v1/deals/DEAL-1/members/2/other",
    ] {
        assert!(validate_patch(path, &body).is_err(), "{path}");
    }
    assert!(validate_patch("/api/v1/deals/DEAL-1/members/2", &serde_json::json!([])).is_err());
    assert!(validate_patch(
        "/api/v1/deals/DEAL-1/members/2",
        &serde_json::json!({"name":"x".repeat(17000)})
    )
    .is_err());
}
