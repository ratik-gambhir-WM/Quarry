use std::{
    env,
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;
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
#[path = "../../../tests/unit/adapters/office/mechanism_tests.rs"]
mod tests;
