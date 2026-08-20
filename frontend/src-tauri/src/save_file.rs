use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use serde::Deserialize;
use tauri::{AppHandle, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use crate::errors::{AppError, AppResult};

const MAX_EXPORT_BYTES: usize = 5 * 1024 * 1024;
const MAX_TITLE_CHARS: usize = 80;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFileInput {
    pub contents: String,
    pub extensions: Vec<String>,
    pub mime_type: String,
    pub suggested_name: String,
    pub title: String,
}

#[tauri::command]
pub async fn save_text_file(
    app: AppHandle,
    window: WebviewWindow,
    input: SaveFileInput,
) -> AppResult<bool> {
    verify_window_origin(&window)?;
    validate_input(&input)?;

    tauri::async_runtime::spawn_blocking(move || save_text_file_blocking(&app, input))
        .await
        .map_err(AppError::internal)?
}

fn save_text_file_blocking(app: &AppHandle, input: SaveFileInput) -> AppResult<bool> {
    let extension_refs = input
        .extensions
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let selection = app
        .dialog()
        .file()
        .set_title(input.title)
        .set_file_name(input.suggested_name)
        .add_filter("Supported text file", &extension_refs)
        .blocking_save_file();
    let Some(file_path) = selection else {
        return Ok(false);
    };
    let path = file_path
        .into_path()
        .map_err(|_| AppError::validation("The selected destination is invalid."))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            AppError::validation("The selected filename needs a supported extension.")
        })?;
    if !input.extensions.iter().any(|allowed| allowed == &extension) {
        return Err(AppError::validation(
            "The selected filename uses an unsupported extension.",
        ));
    }

    atomic_write(&path, input.contents.as_bytes()).map_err(AppError::internal)?;
    Ok(true)
}

fn validate_input(input: &SaveFileInput) -> AppResult<()> {
    if input.contents.trim().is_empty() {
        return Err(AppError::validation("The exported file cannot be empty."));
    }
    if input.contents.len() > MAX_EXPORT_BYTES {
        return Err(AppError::validation(
            "The exported file exceeds the 5 MB limit.",
        ));
    }
    if input.title.trim().is_empty()
        || input.title.chars().count() > MAX_TITLE_CHARS
        || input.title.chars().any(char::is_control)
    {
        return Err(AppError::validation("The save-dialog title is invalid."));
    }
    let file_name = Path::new(&input.suggested_name);
    if file_name.file_name().and_then(|value| value.to_str()) != Some(input.suggested_name.as_str())
    {
        return Err(AppError::validation("The suggested filename is invalid."));
    }
    if input.extensions.is_empty()
        || input.extensions.len() > 4
        || input.extensions.iter().any(|extension| {
            extension.is_empty()
                || extension.len() > 12
                || !extension
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
    {
        return Err(AppError::validation("The file extension list is invalid."));
    }

    let allowed_extensions: &[&str] = match input.mime_type.as_str() {
        "application/json;charset=utf-8" => &["json"],
        "text/markdown;charset=utf-8" => &["md", "markdown"],
        _ => {
            return Err(AppError::validation(
                "The requested file type is not supported.",
            ))
        }
    };
    if input
        .extensions
        .iter()
        .any(|extension| !allowed_extensions.contains(&extension.as_str()))
    {
        return Err(AppError::validation(
            "The file extensions do not match the requested file type.",
        ));
    }
    Ok(())
}

fn verify_window_origin(window: &WebviewWindow) -> AppResult<()> {
    if window.label() != "main" {
        return Err(AppError::permission("This window cannot save files."));
    }

    let url = window.url().map_err(AppError::internal)?;
    let production_origin = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (url.scheme() == "http" && url.host_str() == Some("tauri.localhost"));
    #[cfg(debug_assertions)]
    let development_origin = url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
        && url.port() == Some(1420);
    #[cfg(not(debug_assertions))]
    let development_origin = false;

    if production_origin || development_origin {
        Ok(())
    } else {
        Err(AppError::permission("This origin cannot save files."))
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "invalid export filename".to_string())?;
    let temporary_path =
        path.with_file_name(format!(".{file_name}.{}.quarry-tmp", uuid::Uuid::new_v4()));

    let write_result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)
            .map_err(|error| format!("failed to create temporary export: {error}"))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| format!("failed to write temporary export: {error}"))?;
        fs::rename(&temporary_path, path)
            .map_err(|error| format!("failed to finalize export: {error}"))
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_input() -> SaveFileInput {
        SaveFileInput {
            contents: "# Summary".to_string(),
            extensions: vec!["md".to_string(), "markdown".to_string()],
            mime_type: "text/markdown;charset=utf-8".to_string(),
            suggested_name: "summary.md".to_string(),
            title: "Save summary".to_string(),
        }
    }

    #[test]
    fn rejects_path_components_in_suggested_filename() {
        let mut input = valid_input();
        input.suggested_name = "../summary.md".to_string();
        assert_eq!(
            validate_input(&input).unwrap_err().code,
            crate::errors::ErrorCode::Validation
        );
    }

    #[test]
    fn rejects_mismatched_mime_type_and_extensions() {
        let mut input = valid_input();
        input.extensions = vec!["json".to_string()];
        assert_eq!(
            validate_input(&input).unwrap_err().code,
            crate::errors::ErrorCode::Validation
        );
    }

    #[test]
    fn writes_contents_via_a_sibling_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("summary.md");
        atomic_write(&path, b"# Saved").unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "# Saved");
    }
}
