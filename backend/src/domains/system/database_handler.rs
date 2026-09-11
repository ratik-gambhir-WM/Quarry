use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use super::service::DatabaseService;

#[derive(Clone)]
pub(super) struct SystemHttpState {
    pub database: Arc<DatabaseService>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatabaseStatus {
    pub database_path: String,
}

pub(super) async fn database_status_handler(
    State(state): State<SystemHttpState>,
) -> crate::app::http::error::AppResult<Json<DatabaseStatus>> {
    Ok(Json(DatabaseStatus {
        database_path: state.database.path().display().to_string(),
    }))
}
