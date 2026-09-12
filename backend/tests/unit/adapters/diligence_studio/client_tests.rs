use super::*;

#[test]
fn relative_capability_path_preserves_version_prefix() {
    let client = DiligenceStudioClient::new(
        reqwest::Client::new(),
        Url::parse("http://127.0.0.1:43127/api/v1/").unwrap(),
    );

    assert_eq!(
        client.endpoint("templates/previews").unwrap().as_str(),
        "http://127.0.0.1:43127/api/v1/templates/previews"
    );
}
