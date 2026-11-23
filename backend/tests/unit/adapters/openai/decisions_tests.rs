use super::*;
use crate::adapters::openai::decisions::test_support::fake_provider;
use axum::http::StatusCode;

fn request() -> DecisionRequest<'static> {
    DecisionRequest {
        model: "gpt-6-luna",
        input: vec![InputMessage {
            role: InputRole::User,
            content: "evidence",
        }],
        questions: vec![Question::Predicate {
            name: "damaged".into(),
            instructions: "Is it damaged?".into(),
        }],
    }
}

fn client(endpoint: &str) -> DecisionsClient {
    DecisionsClient::new(reqwest::Client::new(), "test-key").with_endpoint(endpoint)
}

#[tokio::test]
async fn sends_bearer_json_and_decodes_tagged_answers_ignoring_additive_fields() {
    let body = serde_json::json!({
        "model": "gpt-6-luna",
        "answers": [
            {"type": "predicate", "name": "damaged", "probability": 0.95},
            {"type": "choice", "name": "c", "choice": "a", "confidence": 0.8,
             "probabilities": [{"value": "a", "probability": 0.8}, {"value": false, "probability": 0.2}]},
            {"type": "refusal", "name": null},
            {"type": "score", "name": "s", "score": 1.1, "confidence": 0.5, "probabilities": []}
        ],
        "usage": {"input_tokens": 42, "output_tokens": 0, "total_tokens": 42},
        "future_field": true
    })
    .to_string();
    let provider = fake_provider(StatusCode::OK, vec![], body).await;
    let response = client(provider.endpoint())
        .decide(&request(), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(response.answers.len(), 4);
    assert_eq!(
        response.answers[0],
        Answer::Predicate {
            name: Some("damaged".into()),
            probability: 0.95
        }
    );
    assert!(
        matches!(&response.answers[1], Answer::Choice { probabilities, .. }
        if probabilities[1].value == ChoiceValue::Bool(false))
    );
    assert_eq!(response.answers[2], Answer::Refusal { name: None });
    assert_eq!(response.usage.unwrap().input_tokens, 42);
    let requests = provider.requests.lock().unwrap();
    let (headers, sent) = &requests[0];
    assert_eq!(headers["authorization"], "Bearer test-key");
    assert_eq!(headers["content-type"], "application/json");
    let sent: serde_json::Value = serde_json::from_slice(sent).unwrap();
    assert_eq!(sent["questions"][0]["type"], "predicate");
    assert!(sent.get("response_format").is_none());
}

#[tokio::test]
async fn statuses_map_to_sanitized_retry_categories() {
    for (status, headers, expected) in [
        (
            StatusCode::TOO_MANY_REQUESTS,
            vec![("retry-after", "7")],
            DecisionsError::RateLimited {
                retry_after: Some(Duration::from_secs(7)),
            },
        ),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            vec![("retry-after", "9999")],
            DecisionsError::Unavailable {
                retry_after: Some(Duration::from_secs(60)),
            },
        ),
        (
            StatusCode::BAD_GATEWAY,
            vec![],
            DecisionsError::Unavailable { retry_after: None },
        ),
        (
            StatusCode::UNAUTHORIZED,
            vec![],
            DecisionsError::Unauthorized,
        ),
        (
            StatusCode::BAD_REQUEST,
            vec![],
            DecisionsError::Rejected { status: 400 },
        ),
    ] {
        let provider = fake_provider(status, headers, "secret upstream detail".into()).await;
        let error = client(provider.endpoint())
            .decide(&request(), Duration::from_secs(5))
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("secret"));
    }
    assert!(DecisionsError::Timeout.is_retryable());
    assert!(!DecisionsError::Unauthorized.is_retryable());
    assert!(!DecisionsError::InvalidResponse.is_retryable());
}

#[tokio::test]
async fn oversized_or_malformed_bodies_are_rejected() {
    let provider = fake_provider(StatusCode::OK, vec![], "x".repeat(MAX_RESPONSE_BYTES + 1)).await;
    assert_eq!(
        client(provider.endpoint())
            .decide(&request(), Duration::from_secs(5))
            .await
            .unwrap_err(),
        DecisionsError::ResponseTooLarge
    );
    let provider = fake_provider(StatusCode::OK, vec![], "{\"answers\":7}".into()).await;
    assert_eq!(
        client(provider.endpoint())
            .decide(&request(), Duration::from_secs(5))
            .await
            .unwrap_err(),
        DecisionsError::InvalidResponse
    );
    let huge = "y".repeat(MAX_REQUEST_BYTES);
    let mut oversized = request();
    oversized.input[0].content = &huge;
    assert_eq!(
        client(provider.endpoint())
            .decide(&oversized, Duration::from_secs(5))
            .await
            .unwrap_err(),
        DecisionsError::RequestTooLarge
    );
}

#[test]
fn retry_after_accepts_delta_seconds_and_http_dates_with_a_bound() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-19T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    assert_eq!(parse_retry_after("3", now), Some(Duration::from_secs(3)));
    assert_eq!(
        parse_retry_after("Thu, 19 Nov 2026 10:00:20 GMT", now),
        Some(Duration::from_secs(20))
    );
    assert_eq!(
        parse_retry_after("Thu, 19 Nov 2026 11:00:00 GMT", now),
        Some(Duration::from_secs(60))
    );
    assert_eq!(parse_retry_after("soon", now), None);
}

#[test]
fn debug_output_redacts_the_key() {
    let debug = format!(
        "{:?}",
        DecisionsClient::new(reqwest::Client::new(), "sk-secret")
    );
    assert!(!debug.contains("sk-secret"));
    assert!(debug.contains("REDACTED"));
}
