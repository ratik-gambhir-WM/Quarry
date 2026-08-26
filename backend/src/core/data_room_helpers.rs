use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
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

pub fn convert_office_to_pdf(path: &Path) -> Result<Vec<u8>, String> {
    let converter = find_soffice().ok_or_else(|| {
        "Office preview conversion is unavailable because LibreOffice/soffice was not found. Install LibreOffice or set QUARRY_SOFFICE to its executable path.".to_string()
    })?;
    let temp_root = unique_preview_temp_dir();
    let output_dir = temp_root.join("output");
    let profile_dir = temp_root.join("profile");
    fs::create_dir_all(&output_dir)
        .and_then(|_| fs::create_dir_all(&profile_dir))
        .map_err(|err| format!("failed to create a temporary conversion directory: {err}"))?;
    let profile_url = format!("file://{}", profile_dir.display());
    let output = Command::new(&converter)
        .arg(format!("-env:UserInstallation={profile_url}"))
        .arg("--headless")
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(&output_dir)
        .arg(path)
        .env("XDG_CACHE_HOME", &profile_dir)
        .output();
    let result = match output {
        Ok(output) if output.status.success() => (|| {
            let generated_pdf = fs::read_dir(&output_dir)
                .map_err(|err| format!("failed to inspect converted PDF output: {err}"))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|candidate| {
                    candidate
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
                })
                .ok_or_else(|| "LibreOffice completed without producing a PDF".to_string())?;
            fs::read(&generated_pdf)
                .map_err(|err| format!("failed to read the converted PDF: {err}"))
        })(),
        Ok(output) => Err(format!(
            "LibreOffice could not convert this document (exit {}). {} {}",
            output.status,
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(err) => Err(format!(
            "failed to start Office preview converter ({}): {err}",
            converter.display()
        )),
    };
    let _ = fs::remove_dir_all(&temp_root);
    result
}

pub fn convert_office_bytes_to_pdf(extension: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    if !matches!(
        extension.as_str(),
        "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx"
    ) {
        return Err(format!(
            "Office preview conversion does not support .{extension} files"
        ));
    }
    if bytes.is_empty() {
        return Err("the stored document is empty".to_string());
    }

    let temp_root = unique_preview_temp_dir();
    let source_path = temp_root.join(format!("document.{extension}"));
    let result = (|| {
        fs::create_dir_all(&temp_root)
            .map_err(|err| format!("failed to create a temporary document directory: {err}"))?;
        fs::write(&source_path, bytes)
            .map_err(|err| format!("failed to stage the stored document for conversion: {err}"))?;
        convert_office_to_pdf(&source_path)
    })();
    let _ = fs::remove_dir_all(&temp_root);
    result
}

fn find_soffice() -> Option<PathBuf> {
    static SOFFICE_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

    SOFFICE_PATH.get_or_init(discover_soffice).clone()
}

fn discover_soffice() -> Option<PathBuf> {
    if let Some(configured) = env::var_os("QUARRY_SOFFICE") {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Some(candidate) = find_office_executable_on_path(env::var_os("PATH").as_deref()) {
        return Some(candidate);
    }

    [
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
        "/Applications/LibreOfficeDev.app/Contents/MacOS/soffice",
        "/opt/homebrew/bin/soffice",
        "/usr/local/bin/soffice",
        "/usr/bin/soffice",
        "/usr/bin/libreoffice",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
    .or_else(find_office_executable_from_login_shell)
}

fn find_office_executable_on_path(path: Option<&OsStr>) -> Option<PathBuf> {
    let path = path?;
    env::split_paths(path)
        .flat_map(|directory| [directory.join("soffice"), directory.join("libreoffice")])
        .find(|candidate| candidate.is_file())
}

fn find_office_executable_from_login_shell() -> Option<PathBuf> {
    let shell = env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("/bin/sh"));
    let output = Command::new(shell)
        .args([
            "-lc",
            "command -v soffice 2>/dev/null || command -v libreoffice 2>/dev/null",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
}

fn unique_preview_temp_dir() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "quarry-web-document-preview-{}-{timestamp}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn validates_pdf_bytes_in_one_shared_boundary() {
        assert!(validate_pdf_bytes(b"%PDF-1.4\n", "the PDF").is_ok());
        assert_eq!(
            validate_pdf_bytes(b"not a PDF", "the PDF").unwrap_err(),
            "the PDF does not contain a valid PDF header"
        );
        assert_eq!(
            validate_pdf_bytes(&[], "the PDF").unwrap_err(),
            "the PDF is empty"
        );
    }

    #[test]
    fn finds_both_supported_libreoffice_command_names_on_path() {
        let temp_root = unique_preview_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let soffice = temp_root.join("soffice");
        fs::write(&soffice, b"test executable placeholder").unwrap();
        let path = OsString::from(temp_root.as_os_str());

        assert_eq!(find_office_executable_on_path(Some(&path)), Some(soffice));

        fs::remove_dir_all(temp_root).unwrap();
    }
}
