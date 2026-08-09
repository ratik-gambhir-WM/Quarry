use crate::{
    commands::{CommandResult, CommandResultExt},
    services::research_service::{
        list_summarizable_files, summarize_dir, summarize_paths, SummarizableFile,
    },
    state::AppState,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

const MAX_ACTIVITY_LOG_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SUMMARY_EXPORT_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDemoCommandPayload {
    pub email: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDemoCommandResponse {
    pub message: String,
    pub echoed_email: String,
    pub source: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirPathPayload {
    pub path: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedPathsPayload {
    pub paths: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirPathPayloadResponse {
    pub summary: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveMarkdownSummaryPayload {
    pub summary: String,
}

#[tauri::command]
pub fn login_demo_command(payload: LoginDemoCommandPayload) -> LoginDemoCommandResponse {
    LoginDemoCommandResponse {
        message: format!("Rust received a command from {}", payload.source),
        echoed_email: payload.email,
        source: "tauri-command".to_string(),
    }
}

#[tauri::command]
pub fn list_summary_files(
    state: State<'_, AppState>,
    payload: DirPathPayload,
) -> CommandResult<Vec<SummarizableFile>> {
    let path = validate_authorized_path(state.inner(), &payload.path, Some(true))
        .validation_context("list_summary_files")?;
    list_summarizable_files(path.display().to_string()).command_context("list_summary_files")
}

#[tauri::command]
pub async fn summarize(
    state: State<'_, AppState>,
    payload: DirPathPayload,
) -> CommandResult<String> {
    let path = validate_authorized_path(state.inner(), &payload.path, Some(true))
        .validation_context("summarize")?;
    summarize_dir(path.display().to_string())
        .await
        .command_context("summarize")
}

#[tauri::command]
pub async fn summarize_selected(
    state: State<'_, AppState>,
    payload: SelectedPathsPayload,
) -> CommandResult<String> {
    if payload.paths.is_empty() {
        return Err(crate::errors::AppError::validation(
            "summarize_selected",
            "select at least one file to summarize",
        ));
    }
    let paths = payload
        .paths
        .iter()
        .map(|path| validate_authorized_path(state.inner(), path, Some(false)))
        .collect::<Result<Vec<_>, _>>()
        .validation_context("summarize_selected")?
        .into_iter()
        .map(|path| path.display().to_string())
        .collect();
    summarize_paths(paths)
        .await
        .command_context("summarize_selected")
}

#[tauri::command]
pub async fn save_markdown_summary(
    app: AppHandle,
    payload: SaveMarkdownSummaryPayload,
) -> CommandResult<bool> {
    validate_summary_content(&payload.summary).validation_context("save_markdown_summary")?;
    tauri::async_runtime::spawn_blocking(move || {
        let selection = app
            .dialog()
            .file()
            .set_title("Save markdown summary")
            .set_file_name("summary.md")
            .add_filter("Markdown", &["md", "markdown"])
            .blocking_save_file();
        let Some(file_path) = selection else {
            return Ok(false);
        };
        let path = file_path
            .into_path()
            .map_err(|_| "the selected summary destination is invalid".to_string())?;
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("md" | "markdown")
        ) {
            return Err("summaries must be exported as a .md or .markdown file".to_string());
        }
        atomic_write(&path, payload.summary.as_bytes())?;
        Ok(true)
    })
    .await
    .map_err(|error| format!("summary export worker failed: {error}"))
    .and_then(|result| result)
    .command_context("save_markdown_summary")
}

#[tauri::command]
pub async fn select_summary_source(
    app: AppHandle,
    state: State<'_, AppState>,
    directory: bool,
) -> CommandResult<Option<String>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let picker = app.dialog().file().set_title(if directory {
            "Choose a folder to summarize"
        } else {
            "Choose a file to summarize"
        });
        let selection = if directory {
            picker.blocking_pick_folder()
        } else {
            picker.blocking_pick_file()
        };
        let Some(selection) = selection else {
            return Ok(None);
        };
        let path = selection
            .into_path()
            .map_err(|_| "the selected summary source is invalid".to_string())?
            .canonicalize()
            .map_err(|_| "the selected summary source is unavailable".to_string())?;
        if directory != path.is_dir() {
            return Err("the selected summary source has the wrong type".to_string());
        }
        state.grant_paths([path.clone()])?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|error| format!("summary file picker worker failed: {error}"))
    .and_then(|result| result)
    .command_context("select_summary_source")
}

fn validate_authorized_path(
    state: &AppState,
    raw_path: &str,
    expect_directory: Option<bool>,
) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw_path)
        .canonicalize()
        .map_err(|_| "the selected summary source is unavailable".to_string())?;
    if !state.is_path_authorized(&path)? {
        return Err("use the native picker to authorize this summary source".to_string());
    }
    if let Some(expect_directory) = expect_directory {
        if expect_directory && !path.is_dir() {
            return Err("the selected summary source is not a folder".to_string());
        }
        if !expect_directory && !path.is_file() {
            return Err("the selected summary source is not a file".to_string());
        }
    }
    Ok(path)
}

fn validate_summary_content(summary: &str) -> Result<(), String> {
    if summary.trim().is_empty() {
        return Err("summary cannot be empty".to_string());
    }
    if summary.len() > MAX_SUMMARY_EXPORT_BYTES {
        return Err("summary exceeds the 5 MB export limit".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn export_activity_log(app: AppHandle, payload: String) -> CommandResult<bool> {
    validate_activity_log_payload(&payload).validation_context("export_activity_log")?;

    tauri::async_runtime::spawn_blocking(move || {
        let selection = app
            .dialog()
            .file()
            .set_title("Export Quarry activity log")
            .set_file_name("quarry-session-log.json")
            .add_filter("JSON", &["json"])
            .blocking_save_file();
        let Some(file_path) = selection else {
            return Ok(false);
        };
        let path = file_path
            .into_path()
            .map_err(|_| "the selected export destination is invalid".to_string())?;
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            return Err("activity logs must be exported as a .json file".to_string());
        }
        atomic_write(&path, payload.as_bytes())?;
        Ok(true)
    })
    .await
    .map_err(|err| format!("activity log export worker failed: {err}"))
    .and_then(|result| result)
    .command_context("export_activity_log")
}

fn validate_activity_log_payload(payload: &str) -> Result<(), String> {
    if payload.len() > MAX_ACTIVITY_LOG_BYTES {
        return Err("activity log exceeds the 2 MB export limit".to_string());
    }

    let parsed: serde_json::Value =
        serde_json::from_str(payload).map_err(|_| "activity log must be valid JSON".to_string())?;
    if !parsed
        .get("entries")
        .is_some_and(serde_json::Value::is_array)
    {
        return Err("activity log must contain an entries array".to_string());
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "the selected export filename is invalid".to_string())?;
    let temporary_path = path.with_file_name(format!(".{file_name}.quarry-tmp"));

    let write_result = (|| {
        let mut file = fs::File::create(&temporary_path)
            .map_err(|_| "failed to create the activity log export".to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "failed to write the activity log export".to_string())?;
        fs::rename(&temporary_path, path)
            .map_err(|_| "failed to finalize the activity log export".to_string())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

#[cfg(test)]
#[path = "../../tests/commands/research_tests.rs"]
mod tests;
