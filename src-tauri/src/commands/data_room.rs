use crate::{
    commands::{CommandResult, CommandResultExt},
    services::data_room_service::{
        build_document_preview, list_deal_data_room as list_deal_data_room_in_service,
        DealDataRoom, DocumentPreview,
    },
    state::AppState,
};
use tauri::State;

#[tauri::command]
pub async fn list_deal_data_room(
    state: State<'_, AppState>,
    deal_id: String,
) -> CommandResult<DealDataRoom> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || list_deal_data_room_in_service(&state, deal_id))
        .await
        .map_err(|err| format!("data-room listing worker failed: {err}"))
        .and_then(|result| result)
        .command_context("list_deal_data_room")
}

#[tauri::command]
pub async fn preview_deal_document(
    state: State<'_, AppState>,
    deal_id: String,
    relative_path: String,
) -> CommandResult<DocumentPreview> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        build_document_preview(&state, &deal_id, &relative_path)
    })
    .await
    .map_err(|err| format!("document preview worker failed: {err}"))
    .and_then(|result| result)
    .command_context("preview_deal_document")
}

#[cfg(test)]
#[path = "../../tests/commands/data_room_tests.rs"]
mod tests;
