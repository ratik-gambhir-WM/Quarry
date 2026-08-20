use axum::Json;

pub(crate) async fn health_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

pub(crate) async fn capabilities_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "apiVersion": "v1",
        "features": {
            "documentJobEvents": true,
            "multipartDealUploads": true,
            "remoteDocumentPreview": true
        }
    }))
}
