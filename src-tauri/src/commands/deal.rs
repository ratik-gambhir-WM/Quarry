use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    commands::{AppError, CommandResult, CommandResultExt},
    repository::deal_repository::{
        archive_deal as archive_deal_in_repository,
        get_deal_with_metadata as get_deal_with_metadata_in_repository,
        list_deals as list_deals_in_repository, Deal, DealWithMetadata,
    },
    services::deal_service::{
        extract_deal_questions_and_thesis_for_selected_files,
        save_deal_and_extract as save_deal_and_extract_in_service,
        ExtractDealQuestionsAndThesisInput, SaveDealAndExtractInput, SaveDealAndExtractResponse,
        SaveDealAndFindFilesResponse,
    },
    state::AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub database_path: String,
    pub user_version: i64,
}

#[tauri::command]
pub fn database_status(state: State<'_, AppState>) -> CommandResult<DatabaseStatus> {
    let user_version = state
        .with_sqlite_db(|db| db.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0)))
        .command_context("database_status")?;

    Ok(DatabaseStatus {
        database_path: state
            .sqlite_db_path()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("quarry.sqlite3")
            .to_string(),
        user_version,
    })
}

#[tauri::command]
pub async fn save_deal_and_extract(
    state: State<'_, AppState>,
    input: SaveDealAndExtractInput,
) -> CommandResult<SaveDealAndFindFilesResponse> {
    let canonical_root = std::path::Path::new(input.main_data_room_folder.trim())
        .canonicalize()
        .map_err(|_| {
            AppError::validation(
                "save_deal_and_extract",
                "The selected data room is unavailable.",
            )
        })?;
    if !state
        .is_path_granted(&canonical_root)
        .validation_context("save_deal_and_extract")?
    {
        return Err(AppError::validation(
            "save_deal_and_extract",
            "Choose the data room with the native folder picker.",
        ));
    }
    save_deal_and_extract_in_service(&state, input)
        .await
        .command_context("save_deal_and_extract")
}

#[tauri::command]
pub async fn select_deal_data_room_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(selection) = app
            .dialog()
            .file()
            .set_title("Choose main data room folder")
            .blocking_pick_folder()
        else {
            return Ok(None);
        };
        let path = selection
            .into_path()
            .map_err(|_| "the selected data room path is invalid".to_string())?
            .canonicalize()
            .map_err(|_| "the selected data room is unavailable".to_string())?;
        if !path.is_dir() {
            return Err("the selected data room is not a folder".to_string());
        }
        state.grant_paths([path.clone()])?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|error| format!("data room picker worker failed: {error}"))
    .and_then(|result| result)
    .command_context("select_deal_data_room_folder")
}

#[tauri::command]
pub async fn extract_deal_questions_and_thesis(
    state: State<'_, AppState>,
    input: ExtractDealQuestionsAndThesisInput,
) -> CommandResult<SaveDealAndExtractResponse> {
    extract_deal_questions_and_thesis_for_selected_files(&state, input)
        .await
        .command_context("extract_deal_questions_and_thesis")
}

#[tauri::command]
pub async fn list_deals(state: State<'_, AppState>) -> CommandResult<Vec<DealWithMetadata>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || list_deals_in_repository(&state))
        .await
        .map_err(|err| format!("deal listing worker failed: {err}"))
        .and_then(|result| result)
        .command_context("list_deals")
}

#[tauri::command]
pub async fn get_deal(state: State<'_, AppState>, deal_id: i64) -> CommandResult<DealWithMetadata> {
    if deal_id <= 0 {
        return Err(AppError::validation(
            "get_deal",
            "dealId must be a positive integer",
        ));
    }

    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        get_deal_with_metadata_in_repository(&state, deal_id)
    })
    .await
    .map_err(|err| format!("deal lookup worker failed: {err}"))
    .and_then(|result| result)
    .and_then(|deal| deal.ok_or_else(|| "deal was not found".to_string()))
    .command_context("get_deal")
}

#[tauri::command]
pub async fn archive_deal(state: State<'_, AppState>, deal_id: i64) -> CommandResult<Deal> {
    if deal_id <= 0 {
        return Err(AppError::validation(
            "archive_deal",
            "dealId must be a positive integer",
        ));
    }

    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || archive_deal_in_repository(&state, deal_id))
        .await
        .map_err(|err| format!("deal archive worker failed: {err}"))
        .and_then(|result| result)
        .and_then(|deal| deal.ok_or_else(|| "deal was not found".to_string()))
        .command_context("archive_deal")
}
