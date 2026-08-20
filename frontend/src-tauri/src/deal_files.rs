use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use walkdir::WalkDir;

use crate::{
    errors::{AppError, AppResult},
    security::verify_main_window_origin,
};

const MAX_SELECTED_FILE_BYTES: usize = 50 * 1024 * 1024;
const MAX_TOTAL_SELECTED_BYTES: usize = 50 * 1024 * 1024;
const MAX_SELECTED_FILES: usize = 2;
const SOW_MATCH_TERMS: [&str; 2] = ["sow", "scope of work"];
const PROJECT_TIMELINE_MATCH_TERMS: [&str; 6] = [
    "project timeline",
    "timeline",
    "project plan",
    "workplan",
    "work plan",
    "schedule",
];

#[derive(Clone, Default)]
pub struct LocalDealRoots {
    roots: Arc<Mutex<HashSet<PathBuf>>>,
}

impl LocalDealRoots {
    fn authorize(&self, root: PathBuf) -> AppResult<()> {
        self.roots
            .lock()
            .map_err(|_| internal_error("local deal root registry lock was poisoned"))?
            .insert(root);
        Ok(())
    }

    fn ensure_authorized(&self, root: &Path) -> AppResult<()> {
        let authorized = self
            .roots
            .lock()
            .map_err(|_| internal_error("local deal root registry lock was poisoned"))?
            .contains(root);
        if authorized {
            Ok(())
        } else {
            Err(AppError::permission(
                "Choose this data room folder before reading source files.",
            ))
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDealSourceFile {
    path: String,
    filename: String,
    relative_path: String,
    size_bytes: u64,
    matched_on: Vec<String>,
    mime_type: String,
    text_extracted: bool,
    text_truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDealDataRoom {
    root_path: String,
    root_name: String,
    files: Vec<LocalDealSourceFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadDealSourceFilesInput {
    root_path: String,
    paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalDealFileContents {
    path: String,
    filename: String,
    relative_path: String,
    mime_type: String,
    size_bytes: u64,
    data_base64: String,
}

#[tauri::command]
pub async fn select_deal_data_room(
    app: AppHandle,
    window: WebviewWindow,
    roots: State<'_, LocalDealRoots>,
) -> AppResult<Option<LocalDealDataRoom>> {
    verify_main_window_origin(&window)?;
    let selected = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose main data room folder")
            .blocking_pick_folder()
    })
    .await
    .map_err(internal_error)?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let selected = selected
        .into_path()
        .map_err(|_| AppError::validation("The selected data room folder is invalid."))?;
    let data_room = tauri::async_runtime::spawn_blocking(move || scan_data_room(&selected))
        .await
        .map_err(internal_error)??;
    roots.authorize(PathBuf::from(&data_room.root_path))?;
    Ok(Some(data_room))
}

#[tauri::command]
pub async fn read_deal_source_files(
    window: WebviewWindow,
    roots: State<'_, LocalDealRoots>,
    input: ReadDealSourceFilesInput,
) -> AppResult<Vec<LocalDealFileContents>> {
    verify_main_window_origin(&window)?;
    if input.paths.is_empty() || input.paths.len() > MAX_SELECTED_FILES {
        return Err(AppError::validation("Select one or two deal source files."));
    }

    let registry = roots.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let root = canonical_data_room_root(Path::new(input.root_path.trim()))?;
        registry.ensure_authorized(&root)?;
        read_selected_files(&root, &input.paths)
    })
    .await
    .map_err(internal_error)?
}

fn scan_data_room(root: &Path) -> AppResult<LocalDealDataRoom> {
    let root = canonical_data_room_root(root)?;
    let admin_roots = admin_search_roots(&root);
    let mut files = Vec::new();
    if admin_roots.is_empty() {
        collect_matching_files(&root, &root, &mut files)?;
    } else {
        for admin_root in &admin_roots {
            collect_matching_files(&root, admin_root, &mut files)?;
        }
        if !has_source_type(&files, "SOW") || !has_source_type(&files, "Project Timeline") {
            collect_matching_files(&root, &root, &mut files)?;
        }
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files.dedup_by(|left, right| left.path == right.path);

    let root_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Data room")
        .to_string();
    Ok(LocalDealDataRoom {
        root_path: root.display().to_string(),
        root_name,
        files,
    })
}

fn canonical_data_room_root(root: &Path) -> AppResult<PathBuf> {
    if root.as_os_str().is_empty() {
        return Err(AppError::validation("Choose a data room folder."));
    }
    let canonical = root
        .canonicalize()
        .map_err(|_| AppError::validation("The selected data room folder no longer exists."))?;
    if !canonical.is_dir() {
        return Err(AppError::validation(
            "The selected data room path is not a folder.",
        ));
    }
    Ok(canonical)
}

fn collect_matching_files(
    data_room_root: &Path,
    search_root: &Path,
    files: &mut Vec<LocalDealSourceFile>,
) -> AppResult<()> {
    for entry in WalkDir::new(search_root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || is_ignored_file(entry.path()) {
            continue;
        }
        let path = entry.path();
        let metadata = fs::metadata(path).map_err(internal_error)?;
        if metadata.len() == 0 {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| AppError::validation("A source filename is not valid UTF-8."))?
            .to_string();
        let matched_on = matching_terms(&filename);
        let Some(mime_type) = supported_mime_type(path) else {
            continue;
        };
        if matched_on.is_empty() {
            continue;
        }
        files.push(LocalDealSourceFile {
            path: path.display().to_string(),
            filename,
            relative_path: display_relative_path(data_room_root, path),
            size_bytes: metadata.len(),
            matched_on,
            mime_type: mime_type.to_string(),
            text_extracted: false,
            text_truncated: false,
        });
    }
    Ok(())
}

fn read_selected_files(
    root: &Path,
    selected_paths: &[String],
) -> AppResult<Vec<LocalDealFileContents>> {
    let mut seen = HashSet::with_capacity(selected_paths.len());
    let mut total_bytes = 0usize;
    let mut files = Vec::with_capacity(selected_paths.len());

    for selected in selected_paths {
        let path = Path::new(selected.trim())
            .canonicalize()
            .map_err(|_| AppError::validation("A selected source file no longer exists."))?;
        if !path.starts_with(root) || !path.is_file() || is_ignored_file(&path) {
            return Err(AppError::permission(
                "A selected source file is outside the chosen data room.",
            ));
        }
        if !seen.insert(path.clone()) {
            continue;
        }
        let mime_type = supported_mime_type(&path)
            .ok_or_else(|| AppError::validation("A selected source file type is unsupported."))?;
        let bytes = fs::read(&path).map_err(internal_error)?;
        if bytes.is_empty() {
            return Err(AppError::validation("A selected source file is empty."));
        }
        if bytes.len() > MAX_SELECTED_FILE_BYTES {
            return Err(AppError::validation(
                "A selected source file exceeds the 50 MB limit.",
            ));
        }
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| AppError::validation("The selected source files are too large."))?;
        if total_bytes > MAX_TOTAL_SELECTED_BYTES {
            return Err(AppError::validation(
                "The selected source files exceed the 50 MB total limit.",
            ));
        }
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| AppError::validation("A source filename is not valid UTF-8."))?
            .to_string();
        files.push(LocalDealFileContents {
            path: path.display().to_string(),
            filename,
            relative_path: display_relative_path(root, &path),
            mime_type: mime_type.to_string(),
            size_bytes: bytes.len() as u64,
            data_base64: general_purpose::STANDARD.encode(bytes),
        });
    }

    Ok(files)
}

fn admin_search_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = WalkDir::new(root)
        .min_depth(1)
        .max_depth(3)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| {
                    name.to_ascii_lowercase()
                        .replace(['.', '_', '-'], " ")
                        .split_whitespace()
                        .any(|part| part == "admin" || part == "administration")
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    roots
}

fn has_source_type(files: &[LocalDealSourceFile], source_type: &str) -> bool {
    files
        .iter()
        .any(|file| file.matched_on.iter().any(|item| item == source_type))
}

fn matching_terms(content: &str) -> Vec<String> {
    let haystack = content.to_ascii_lowercase();
    let mut matches = Vec::new();
    if SOW_MATCH_TERMS.iter().any(|term| haystack.contains(term)) {
        matches.push("SOW".to_string());
    }
    if PROJECT_TIMELINE_MATCH_TERMS
        .iter()
        .any(|term| haystack.contains(term))
    {
        matches.push("Project Timeline".to_string());
    }
    matches
}

fn supported_mime_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("pdf") => Some("application/pdf"),
        Some("txt") => Some("text/plain"),
        Some("md") => Some("text/markdown"),
        Some("json") => Some("application/json"),
        Some("html") => Some("text/html"),
        Some("csv") => Some("text/csv"),
        Some("doc") => Some("application/msword"),
        Some("docx") => {
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        Some("pptx") => {
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        Some("xls") => Some("application/vnd.ms-excel"),
        Some("xlsx") => Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        _ => None,
    }
}

fn display_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn is_ignored_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("~$") || name == ".DS_Store")
        .unwrap_or(false)
}

fn internal_error(source: impl std::fmt::Display) -> AppError {
    AppError::internal_operation(
        "deal-files",
        "The local data room could not be accessed.",
        source,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_supported_source_files_in_the_data_room() {
        let directory = tempfile::tempdir().unwrap();
        let admin = directory.path().join("01 Admin");
        fs::create_dir(&admin).unwrap();
        fs::write(admin.join("Project SOW.docx"), b"sow").unwrap();
        fs::write(admin.join("Project Timeline.pdf"), b"timeline").unwrap();
        fs::write(directory.path().join("Financials.xlsx"), b"numbers").unwrap();

        let result = scan_data_room(directory.path()).unwrap();

        assert_eq!(result.files.len(), 2);
        assert_eq!(result.files[0].relative_path, "01 Admin/Project SOW.docx");
    }

    #[test]
    fn reads_only_files_inside_an_authorized_root() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("SOW.docx");
        fs::write(&source, b"questions").unwrap();
        let root = directory.path().canonicalize().unwrap();
        let registry = LocalDealRoots::default();
        registry.authorize(root.clone()).unwrap();
        registry.ensure_authorized(&root).unwrap();

        let files = read_selected_files(&root, &[source.display().to_string()]).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].data_base64, "cXVlc3Rpb25z");

        let outside = tempfile::NamedTempFile::new().unwrap();
        assert!(read_selected_files(&root, &[outside.path().display().to_string()]).is_err());
    }
}
