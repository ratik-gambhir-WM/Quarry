use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener, Runtime};

const FRONTEND_TO_BACKEND_EVENT: &str = "login-demo:frontend-request";
const BACKEND_TO_FRONTEND_EVENT: &str = "login-demo:backend-response";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginDemoEventPayload {
    email: String,
    note: String,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginDemoEventResponse {
    message: String,
    echoed_email: String,
    original_note: String,
    source: String,
}

pub fn register_login_demo_events<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();

    app.listen(FRONTEND_TO_BACKEND_EVENT, move |event| {
        println!("HIT TAURI BACKEND DURING EVENT");
        let response = match serde_json::from_str::<LoginDemoEventPayload>(event.payload()) {
            Ok(payload) => LoginDemoEventResponse {
                message: format!("Rust received an event from {}", payload.source),
                echoed_email: payload.email,
                original_note: payload.note,
                source: "tauri-event".to_string(),
            },
            Err(error) => LoginDemoEventResponse {
                message: format!("Rust could not parse the incoming event payload: {error}"),
                echoed_email: String::new(),
                original_note: String::new(),
                source: "tauri-event-error".to_string(),
            },
        };

        let _ = app_handle.emit(BACKEND_TO_FRONTEND_EVENT, response);
    });
}
