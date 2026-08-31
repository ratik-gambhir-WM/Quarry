use axum::{extract::State, Json};
use serde::Serialize;

use crate::handlers::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatabaseStatus {
    pub database_path: String,
}

pub(crate) async fn database_status_handler(
    State(state): State<AppState>,
) -> crate::errors::AppResult<Json<DatabaseStatus>> {
    Ok(Json(DatabaseStatus {
        database_path: state.database.path().display().to_string(),
    }))
}
