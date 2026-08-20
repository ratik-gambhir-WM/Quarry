use axum::{extract::Query, Json};
use serde::{Deserialize, Serialize};

use crate::events::{handle_login_demo_event, LoginDemoEventPayload, LoginDemoEventResponse};

#[derive(Debug, Deserialize)]
pub(crate) struct GreetQuery {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct GreetResponse {
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginDemoPayload {
    pub email: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LoginDemoResponse {
    pub message: String,
    pub echoed_email: String,
    pub source: String,
}

pub(crate) async fn greet_handler(Query(query): Query<GreetQuery>) -> Json<GreetResponse> {
    Json(GreetResponse {
        message: format!("Hello, {}! You've been greeted from Rust!", query.name),
    })
}

pub(crate) async fn login_demo_handler(
    Json(payload): Json<LoginDemoPayload>,
) -> Json<LoginDemoResponse> {
    Json(LoginDemoResponse {
        message: format!("Rust received a request from {}", payload.source),
        echoed_email: payload.email,
        source: "axum-rest".to_string(),
    })
}

pub(crate) async fn login_demo_event_handler(
    Json(payload): Json<LoginDemoEventPayload>,
) -> Json<LoginDemoEventResponse> {
    Json(handle_login_demo_event(payload))
}
