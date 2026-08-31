use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

pub const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CONVERTER_LOG_BYTES: usize = 64 * 1024;
const OFFICE_CONVERSION_TIMEOUT: Duration = Duration::from_secs(45);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug)]
struct BoundedCommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

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

pub fn convert_office_to_pdf(converter: &Path, path: &Path) -> Result<Vec<u8>, String> {
    let temp_root = unique_preview_temp_dir();
    let output_dir = temp_root.join("output");
    let profile_dir = temp_root.join("profile");
    fs::create_dir_all(&output_dir)
        .and_then(|_| fs::create_dir_all(&profile_dir))
        .map_err(|err| format!("failed to create a temporary conversion directory: {err}"))?;
    let profile_url = format!("file://{}", profile_dir.display());
    let mut command = Command::new(converter);
    command
        .arg(format!("-env:UserInstallation={profile_url}"))
        .arg("--headless")
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(&output_dir)
        .arg(path)
        .env("XDG_CACHE_HOME", &profile_dir);
    let output = run_command_with_timeout(&mut command, OFFICE_CONVERSION_TIMEOUT);
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
            read_file_with_limit(&generated_pdf, MAX_PDF_BYTES).map_err(|err| {
                format!("failed to read the converted PDF within the preview limit: {err}")
            })
        })(),
        Ok(output) => Err(format!(
            "LibreOffice could not convert this document (exit {}). {} {}",
            output.status,
            display_bounded_output(&output.stdout, output.stdout_truncated),
            display_bounded_output(&output.stderr, output.stderr_truncated)
        )),
        Err(err) => Err(format!(
            "Office preview converter ({}) failed: {err}",
            converter.display()
        )),
    };
    let _ = fs::remove_dir_all(&temp_root);
    result
}

fn run_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> Result<BoundedCommandOutput, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture converter stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture converter stderr".to_string())?;
    let stdout_reader = thread::spawn(move || drain_bounded(stdout, MAX_CONVERTER_LOG_BYTES));
    let stderr_reader = thread::spawn(move || drain_bounded(stderr, MAX_CONVERTER_LOG_BYTES));
    let started_at = Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started_at.elapsed() < timeout => thread::sleep(PROCESS_POLL_INTERVAL),
            Ok(None) => {
                terminate_process_tree(&mut child);
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!(
                    "Office preview conversion timed out after {} seconds and was terminated",
                    timeout.as_secs()
                ));
            }
            Err(error) => {
                terminate_process_tree(&mut child);
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!(
                    "failed while waiting for Office preview converter: {error}"
                ));
            }
        }
    };
    let (stdout, stdout_truncated) = join_bounded_reader(stdout_reader, "stdout")?;
    let (stderr, stderr_truncated) = join_bounded_reader(stderr_reader, "stderr")?;
    Ok(BoundedCommandOutput {
        status,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

fn terminate_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // SAFETY: the child starts in a process group whose ID is its PID. A
        // negative PID targets only that group, including converter children.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}

fn drain_bounded(mut reader: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut retained = Vec::with_capacity(limit.min(8 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(retained.len());
        let keep = remaining.min(read);
        retained.extend_from_slice(&buffer[..keep]);
        truncated |= keep < read;
    }
    Ok((retained, truncated))
}

fn join_bounded_reader(
    reader: thread::JoinHandle<io::Result<(Vec<u8>, bool)>>,
    stream_name: &str,
) -> Result<(Vec<u8>, bool), String> {
    reader
        .join()
        .map_err(|_| format!("Office preview converter {stream_name} reader panicked"))?
        .map_err(|error| format!("failed to read Office preview converter {stream_name}: {error}"))
}

fn display_bounded_output(bytes: &[u8], truncated: bool) -> String {
    let suffix = if truncated { " [output truncated]" } else { "" };
    format!("{}{suffix}", String::from_utf8_lossy(bytes).trim())
}

fn read_file_with_limit(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let metadata = path
        .metadata()
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if metadata.len() > limit {
        return Err(format!(
            "converted output is {} bytes; limit is {limit} bytes",
            metadata.len()
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if bytes.len() as u64 > limit {
        return Err(format!("converted output exceeds the {limit} byte limit"));
    }
    Ok(bytes)
}

pub fn convert_office_bytes_to_pdf(
    converter: &Path,
    extension: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, String> {
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
        convert_office_to_pdf(converter, &source_path)
    })();
    let _ = fs::remove_dir_all(&temp_root);
    result
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
    use std::io::Cursor;

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
    fn converter_logs_are_drained_but_retained_only_up_to_the_limit() {
        let (retained, truncated) = drain_bounded(Cursor::new(b"0123456789"), 4).unwrap();

        assert_eq!(retained, b"0123");
        assert!(truncated);
    }

    #[cfg(unix)]
    #[test]
    fn converter_process_is_terminated_at_the_hard_timeout() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 2"]);

        let error = run_command_with_timeout(&mut command, Duration::from_millis(50)).unwrap_err();

        assert!(error.contains("timed out"));
        assert!(error.contains("terminated"));
    }
}
