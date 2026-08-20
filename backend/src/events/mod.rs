use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDemoEventPayload {
    pub email: String,
    pub note: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDemoEventResponse {
    pub message: String,
    pub echoed_email: String,
    pub original_note: String,
    pub source: String,
}

pub fn handle_login_demo_event(payload: LoginDemoEventPayload) -> LoginDemoEventResponse {
    LoginDemoEventResponse {
        message: format!("Rust received an event from {}", payload.source),
        echoed_email: payload.email,
        original_note: payload.note,
        source: "axum-event".to_string(),
    }
}
