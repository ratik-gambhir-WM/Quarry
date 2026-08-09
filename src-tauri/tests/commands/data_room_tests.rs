use std::path::Path;

use super::*;
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{get_ipc_response, mock_builder, mock_context, noop_assets, INVOKE_KEY},
    webview::InvokeRequest,
    WebviewWindowBuilder,
};

use crate::state::AppState;

const ACTUAL_PDF_RELATIVE_PATH: &str = "4 Security and Compliance/4.1 Cybersecurity/4.1.2 Cybersecurity testing and remediation/BetaNXT Standard - Application Security Testing.pdf";

#[test]
fn ipc_accepts_frontend_argument_names_and_returns_pdf_shape() {
    let state = AppState::new_for_test().expect("test state should initialize");
    let Ok(data_room) = list_deal_data_room_in_service(&state, "project-alpha".to_string()) else {
        return;
    };
    if !Path::new(&data_room.root_path)
        .join(ACTUAL_PDF_RELATIVE_PATH)
        .is_file()
    {
        return;
    }

    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            list_deal_data_room,
            preview_deal_document
        ])
        .build(mock_context(noop_assets()))
        .expect("mock Tauri app should build");
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("mock webview should build");
    let listing = get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: "list_deal_data_room".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(serde_json::json!({
                "dealId": "project-alpha",
            })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .expect("frontend-shaped listing request should succeed")
    .deserialize::<serde_json::Value>()
    .expect("listing response should be JSON");
    let selected_relative_path = find_relative_path(&listing["tree"], ACTUAL_PDF_RELATIVE_PATH)
        .expect("actual PDF should be discoverable with its relative path");

    let response = get_ipc_response(
        &webview,
        InvokeRequest {
            cmd: "preview_deal_document".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(serde_json::json!({
                "dealId": "project-alpha",
                "relativePath": selected_relative_path,
            })),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .expect("frontend-shaped IPC request should succeed")
    .deserialize::<serde_json::Value>()
    .expect("IPC response should be JSON");

    assert_eq!(response["mimeType"], "application/pdf");
    assert_eq!(response["sourceKind"], "native");
    assert!(response["pdfBase64"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
}

fn find_relative_path<'a>(nodes: &'a serde_json::Value, expected: &str) -> Option<&'a str> {
    for node in nodes.as_array()? {
        if node["relativePath"].as_str() == Some(expected) {
            return node["relativePath"].as_str();
        }
        if let Some(found) = find_relative_path(&node["children"], expected) {
            return Some(found);
        }
    }

    None
}
