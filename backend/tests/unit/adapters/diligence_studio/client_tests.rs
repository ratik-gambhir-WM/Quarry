use super::*;

use reqwest::header::CONTENT_TYPE;

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

#[test]
fn requests_apply_fixed_defaults_and_allow_operation_specific_configuration() {
    let client = DiligenceStudioClient::new(
        reqwest::Client::new(),
        Url::parse("http://127.0.0.1:43127/api/v1/").unwrap(),
    );
    let endpoint = client.endpoint("import").unwrap();
    let custom_timeout = Duration::from_secs(5);

    let request = client
        .request(Method::POST, endpoint, |request| {
            request
                .header(APP_ID_HEADER, "ignored-custom-app")
                .header(CONTENT_TYPE, "application/custom")
                .timeout(custom_timeout)
        })
        .build()
        .unwrap();

    assert_eq!(request.timeout(), Some(&custom_timeout));
    assert_eq!(
        request
            .headers()
            .get(APP_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
        Some(DILIGENCE_STUDIO_APP_ID)
    );
    assert_eq!(
        request
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/custom")
    );

    let default_request = client
        .request(
            Method::GET,
            client.endpoint("templates/previews").unwrap(),
            |request| request,
        )
        .build()
        .unwrap();
    assert_eq!(default_request.timeout(), Some(&REQUEST_TIMEOUT));
}
