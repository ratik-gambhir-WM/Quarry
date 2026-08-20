use super::*;

#[tokio::test]
async fn direct_graph_operations_require_credentials() {
    let client = SharePointClient::new(SharePointClientConfig::default());
    let error = client
        .get_drive_id(&TeamIdentifier::new("team-1"), None)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("tenantId, clientId, and clientSecret are required"));
}
