use axum::{extract::State, Json};
use serde::Serialize;

use crate::handlers::{run_blocking, AppError, AppState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatabaseStatus {
    pub database_path: String,
    pub user_version: i64,
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
    let user_version =
        state.with_db(|db| db.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0)))?;

    Ok(DatabaseStatus {
        database_path: state.db_path().display().to_string(),
        user_version,
    })
}
