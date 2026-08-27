use axum::{extract::State, Json};
use serde::Serialize;

use crate::handlers::{run_blocking, AppError, AppState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatabaseStatus {
    pub database_path: String,
}

pub(crate) async fn database_status_handler(
    State(state): State<AppState>,
) -> crate::errors::AppResult<Json<DatabaseStatus>> {
    run_blocking(move || database_status(&state))
        .await
        .map(Json)
        .map_err(AppError::internal)
}

fn database_status(state: &AppState) -> Result<DatabaseStatus, String> {
    Ok(DatabaseStatus {
        database_path: state.db_path().display().to_string(),
    })
}
