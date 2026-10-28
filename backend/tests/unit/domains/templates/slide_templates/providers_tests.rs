use super::*;
#[test]
fn provider_errors_are_sanitized_and_stage_specific() {
    assert!(translate_http_error(429, "embedding").retryable);
    assert_eq!(
        translate_http_error(503, "classification").code,
        "classification_unavailable"
    );
    assert!(!translate_http_error(401, "classification").retryable);
    for (response, code) in [
        (
            json!({"output":[],"status":"incomplete"}),
            "classification_incomplete",
        ),
        (
            json!({"output":[],"status":"completed"}),
            "classification_missing_output",
        ),
        (
            json!({"output":[{"type":"message","content":[{"type":"refusal"}]}],"status":"completed"}),
            "classification_refused",
        ),
        (
            json!({"output":[{"type":"message","content":[{"type":"output_text","text":"{}"}]}],"status":"completed"}),
            "classification_invalid_output",
        ),
    ] {
        let error = OpenAISlideClassifier::parse_response(response).unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.to_string(), "The slide provider operation failed.");
    }
    assert_eq!(
        OpenAISlideEmbedder::parse_response(json!({"data":[{"embedding":[1,0]}]}), Some(2))
            .unwrap(),
        [1.0, 0.0]
    );
    for response in [
        json!({"data":[]}),
        json!({"data":[{"embedding":[0,0]}]}),
        json!({"data":[{"embedding":[1]}]}),
    ] {
        assert!(OpenAISlideEmbedder::parse_response(response, Some(2)).is_err());
    }
}
#[test]
fn classifier_request_matches_nonstored_schema_contract() {
    let classifier = OpenAISlideClassifier::new(
        reqwest::Client::builder()
            .retry(reqwest::retry::never())
            .build()
            .unwrap(),
        OpenAISlideClassifierOptions {
            api_key: "test".to_owned(),
            base_url: None,
            model: "configured".to_owned(),
            timeout: Duration::from_secs(1),
            max_output_tokens: 900,
        },
    );
    let body = classifier.request_body(&SlideClassificationInput {
        digest: "bounded digest".to_owned(),
        preview: Some(b"png".to_vec()),
    });
    assert_eq!(body["store"], false);
    assert_eq!(body["tools"], json!([]));
    assert_eq!(body["max_output_tokens"], 900);
    assert_eq!(
        body["input"][0]["content"][1]["image_url"],
        "data:image/png;base64,cG5n"
    );
    assert_eq!(
        body["text"]["format"]["schema"]["additionalProperties"],
        false
    );
}

#[test]
fn provider_responses_validate_the_current_nested_metadata_contract() {
    let metadata = json!({
        "subject":{"summary":"Security testing finding.","domains":[{"id":"cybersecurity","relevance":"primary","topics":["security-testing"]}],"otherTopics":[],"technologies":[],"entities":[],"claims":[],"synonyms":[]},
        "communication":{"intents":["finding"],"informationTypes":[],"audience":[]},
        "templateFit":{"archetype":"finding","contentSlots":[]},
        "visual":{"layoutType":"diagram-led","visualElements":[],"contentDensity":"low","structuralFeatures":[]},
        "retrievalKeywords":["security"]
    });
    let response = json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":metadata.to_string()}]}]});
    assert_eq!(
        OpenAISlideClassifier::parse_response(response)
            .unwrap()
            .subject
            .summary,
        "Security testing finding."
    );
    for response in [
        json!({"output":[],"status":"incomplete"}),
        json!({"output":[],"status":"completed"}),
    ] {
        assert!(OpenAISlideClassifier::parse_response(response).is_err());
    }
    assert_eq!(
        OpenAISlideEmbedder::parse_response(json!({"data":[{"embedding":[1,0]}]}), Some(2))
            .unwrap(),
        [1.0, 0.0]
    );
}

#[tokio::test]
async fn provider_http_requests_match_reference_and_never_retry() {
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode},
        routing::post,
        Json, Router,
    };
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let router = Router::new()
        .route(
            "/{endpoint}",
            post(
                async move |State(sender): State<
                    tokio::sync::mpsc::UnboundedSender<(HeaderMap, Value)>,
                >,
                            headers: HeaderMap,
                            Json(body): Json<Value>| {
                    sender.send((headers, body)).unwrap();
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(json!({"error":{"message":"private upstream body"}})),
                    )
                },
            ),
        )
        .with_state(sender);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let classifier = OpenAISlideClassifier::new(
        reqwest::Client::builder()
            .retry(reqwest::retry::never())
            .build()
            .unwrap(),
        OpenAISlideClassifierOptions {
            api_key: "test".to_owned(),
            base_url: Some(base_url.clone()),
            model: "configured-classifier".to_owned(),
            timeout: Duration::from_millis(1234),
            max_output_tokens: 900,
        },
    );
    let error = classifier
        .classify(&SlideClassificationInput {
            digest: "bounded digest".to_owned(),
            preview: Some(b"png".to_vec()),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "classification_unavailable");
    assert!(error.retryable);
    let (headers, body) = receiver.recv().await.unwrap();
    assert_eq!(headers["authorization"], "Bearer test");
    assert_eq!(body["model"], "configured-classifier");
    assert_eq!(body["store"], false);
    assert_eq!(body["tools"], json!([]));
    assert_eq!(
        body["text"]["format"]["schema"]["additionalProperties"],
        false
    );
    let embedder = OpenAISlideEmbedder::new(
        reqwest::Client::builder()
            .retry(reqwest::retry::never())
            .build()
            .unwrap(),
        OpenAISlideEmbedderOptions {
            api_key: "test".to_owned(),
            base_url: Some(base_url),
            model: "configured-embedding".to_owned(),
            timeout: Duration::from_millis(200),
            dimensions: Some(2),
            max_input_bytes: 100,
        },
    );
    let error = embedder.embed("canonical document").await.unwrap_err();
    assert_eq!(error.code, "embedding_unavailable");
    assert!(error.retryable);
    let (headers, body) = receiver.recv().await.unwrap();
    assert_eq!(headers["authorization"], "Bearer test");
    assert_eq!(
        body,
        json!({"input":"canonical document","model":"configured-embedding","dimensions":2,"encoding_format":"float"})
    );
    assert!(receiver.try_recv().is_err());
    server.abort();
}

#[tokio::test]
async fn embedding_validates_utf8_byte_limits_before_sending_and_omits_unset_dimensions() {
    let embedder = OpenAISlideEmbedder::new(
        reqwest::Client::builder()
            .retry(reqwest::retry::never())
            .build()
            .unwrap(),
        OpenAISlideEmbedderOptions {
            api_key: "test".to_owned(),
            base_url: Some("http://127.0.0.1:0".to_owned()),
            model: "configured-embedding".to_owned(),
            timeout: Duration::from_millis(200),
            dimensions: None,
            max_input_bytes: 4,
        },
    );
    assert!(embedder.request_body("text").get("dimensions").is_none());
    for text in ["", "\u{feff} ", "🦀 "] {
        let error = embedder.embed(text).await.unwrap_err();
        assert_eq!(error.code, "embedding_invalid_vector");
        assert!(!error.retryable);
    }
}

#[test]
fn provider_schema_fields_map_to_canonical_metadata_without_accepting_conflicts() {
    let metadata = serde_json::json!({"subject":{"summary":"Cloud architecture.","domains":[{"id":"cloud","relevance":"primary","topics":[]}],"other_topics":[],"technologies":[],"entities":[],"claims":[],"synonyms":[]},"communication":{"intents":["explanation"],"information_types":[],"audience":[]},"template_fit":{"archetype":"architecture","content_slots":[]},"visual":{"layout_type":"diagram-led","visual_elements":[],"content_density":"low","structural_features":[]},"retrieval_keywords":[]});
    let response = serde_json::json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":metadata.to_string()}]}]});
    assert_eq!(
        OpenAISlideClassifier::parse_response(response)
            .unwrap()
            .subject
            .summary,
        "Cloud architecture."
    );
    assert!(
        provider_metadata_fields(serde_json::json!({"template_fit":{},"templateFit":{}})).is_err()
    );
}
