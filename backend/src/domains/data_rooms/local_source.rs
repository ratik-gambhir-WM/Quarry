use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;

pub const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataRoomTreeNode {
    pub children: Option<Vec<DataRoomTreeNode>>,
    pub default_expanded: bool,
    pub error: Option<String>,
    pub id: String,
    pub kind: String,
    pub name: String,
    pub relative_path: Option<String>,
}

pub fn build_directory_node(
    root: &Path,
    directory: &Path,
    relative_path: &Path,
    default_expanded: bool,
) -> DataRoomTreeNode {
    let name = if relative_path.as_os_str().is_empty() {
        root.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Data Room")
            .to_string()
    } else {
        directory
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Folder")
            .to_string()
    };
    let id = if relative_path.as_os_str().is_empty() {
        "data-room-root".to_string()
    } else {
        relative_path.to_string_lossy().to_string()
    };

    match fs::read_dir(directory) {
        Ok(entries) => {
            let mut children = Vec::new();
            for entry_result in entries {
                let entry = match entry_result {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };
                let entry_name = entry.file_name().to_string_lossy().to_string();
                if entry_name.starts_with('.') || entry_name.starts_with("~$") {
                    continue;
                }
                let child_relative = relative_path.join(&entry_name);
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(err) => {
                        let mut node = file_node(&entry_name, &child_relative);
                        node.error = Some(format!("This item cannot be read: {err}"));
                        children.push(node);
                        continue;
                    }
                };
                if file_type.is_dir() {
                    children.push(build_directory_node(
                        root,
                        &entry.path(),
                        &child_relative,
                        false,
                    ));
                } else if file_type.is_file() {
                    children.push(file_node(&entry_name, &child_relative));
                }
            }
            children.sort_by(|left, right| {
                let left_folder = left.kind == "folder";
                let right_folder = right.kind == "folder";
                right_folder
                    .cmp(&left_folder)
                    .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            });
            DataRoomTreeNode {
                children: Some(children),
                default_expanded,
                error: None,
                id,
                kind: "folder".to_string(),
                name,
                relative_path: None,
            }
        }
        Err(err) => DataRoomTreeNode {
            children: Some(Vec::new()),
            default_expanded,
            error: Some(format!("This folder cannot be read: {err}")),
            id,
            kind: "folder".to_string(),
            name,
            relative_path: None,
        },
    }
}

fn file_node(name: &str, relative_path: &Path) -> DataRoomTreeNode {
    let extension = relative_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kind = match extension.as_str() {
        "pdf" => "pdf",
        "xlsx" => "sheet",
        _ => "doc",
    };
    let relative_path = relative_path.to_string_lossy().to_string();
    DataRoomTreeNode {
        children: None,
        default_expanded: false,
        error: None,
        id: relative_path.clone(),
        kind: kind.to_string(),
        name: name.to_string(),
        relative_path: Some(relative_path),
    }
}

pub fn resolve_relative_file(root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative_path);
    if relative_path.trim().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(
            "document path must be a non-empty path relative to the deal data room".to_string(),
        );
    }
    let canonical_candidate = root.join(relative).canonicalize().map_err(|err| {
        format!(
            "the selected document is inaccessible ({}): {err}",
            relative.display()
        )
    })?;
    if !canonical_candidate.starts_with(root) {
        return Err("the selected document is outside the configured deal data room".to_string());
    }
    if !canonical_candidate.is_file() {
        return Err(format!(
            "the selected path is not a file: {}",
            relative.display()
        ));
    }
    Ok(canonical_candidate)
}

pub fn read_pdf(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = path.metadata().map_err(|err| {
        format!(
            "failed to inspect the selected PDF ({}): {err}",
            path.display()
        )
    })?;
    if metadata.len() > MAX_PDF_BYTES {
        return Err(format!(
            "The PDF is too large to preview ({} MB; limit is {} MB).",
            metadata.len() / (1024 * 1024),
            MAX_PDF_BYTES / (1024 * 1024)
        ));
    }
    let bytes = fs::read(path).map_err(|err| {
        format!(
            "failed to read the selected PDF ({}): {err}",
            path.display()
        )
    })?;
    validate_pdf_bytes(&bytes, "the selected PDF")?;
    Ok(bytes)
}

pub fn validate_pdf_bytes(bytes: &[u8], subject: &str) -> Result<(), String> {
    if bytes.is_empty() {
        return Err(format!("{subject} is empty"));
    }
    if bytes.len() as u64 > MAX_PDF_BYTES {
        return Err(format!(
            "{subject} is too large to preview ({} MB; limit is {} MB).",
            bytes.len() / (1024 * 1024),
            MAX_PDF_BYTES / (1024 * 1024)
        ));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(format!("{subject} does not contain a valid PDF header"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/data_rooms/local_source_tests.rs"]
mod tests;
