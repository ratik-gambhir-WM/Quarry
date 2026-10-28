//! Best-effort first-slide preview generation.

mod headless;
pub use headless::HeadlessTemplatePreviewGenerator;

use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use tempfile::tempdir;

use super::RequestCancellation;
use serde_json::Value;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(PNG_SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

const QUICK_LOOK_MAX_PROCESS_OUTPUT: usize = 32 * 1024;

/// Reap the process on every error path, including a failure to start a pipe reader.
struct PreviewProcess(Child);
impl Drop for PreviewProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn drain_process_output(
    mut pipe: impl Read + Send + 'static,
    exceeded_limit: Arc<AtomicBool>,
) -> std::io::Result<thread::JoinHandle<std::io::Result<()>>> {
    thread::Builder::new()
        .name("quicklook-output".into())
        .spawn(move || {
            let mut buffer = [0_u8; 4096];
            let mut count = 0;
            let mut retained = 0;
            loop {
                let read = match pipe.read(&mut buffer[retained..]) {
                    Ok(0) => {
                        // Node's UTF-8 StringDecoder flushes an incomplete final sequence
                        // as one replacement character, whose encoded length is three.
                        if retained > 0 && count + 3 > QUICK_LOOK_MAX_PROCESS_OUTPUT {
                            exceeded_limit.store(true, Ordering::Release);
                        }
                        return Ok(());
                    }
                    Ok(read) => read,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        exceeded_limit.store(true, Ordering::Release);
                        return Err(error);
                    }
                };
                // execFile uses encoding: "utf8" and measures Buffer.byteLength of
                // decoded chunks. Count replacement characters without collecting
                // output, retaining at most three bytes between reads.
                let available = retained + read;
                retained = 0;
                let mut offset = 0;
                while offset < available {
                    match std::str::from_utf8(&buffer[offset..available]) {
                        Ok(_) => {
                            count += available - offset;
                            break;
                        }
                        Err(error) => {
                            count += error.valid_up_to();
                            offset += error.valid_up_to();
                            if let Some(length) = error.error_len() {
                                count += 3;
                                offset += length;
                            } else {
                                retained = available - offset;
                                buffer.copy_within(offset..available, 0);
                                break;
                            }
                        }
                    }
                }
                if count > QUICK_LOOK_MAX_PROCESS_OUTPUT {
                    exceeded_limit.store(true, Ordering::Release);
                    return Ok(());
                }
            }
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedTemplatePreview {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub trait TemplatePreviewGenerator: Send + Sync {
    fn requires_document(&self) -> bool {
        false
    }
    fn supports_independent_slides(&self) -> bool {
        true
    }
    fn generate_document_cancellable(
        &self,
        source: &[u8],
        document: &Value,
        cancellation: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        if cancellation.is_some_and(RequestCancellation::is_cancelled) {
            return Ok(None);
        }
        self.generate_document(source, document)
    }
    fn generate_document(
        &self,
        source: &[u8],
        _document: &Value,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        self.generate(source)
    }

    fn generate(&self, source: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>>;
}

#[derive(Default)]
pub struct DisabledTemplatePreviewGenerator;

impl TemplatePreviewGenerator for DisabledTemplatePreviewGenerator {
    fn generate(&self, _source: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        Ok(None)
    }
}

#[derive(Clone, Debug)]
pub struct QuickLookTemplatePreviewGenerator {
    executable_path: PathBuf,
    max_bytes: usize,
    max_dimension: u32,
    render_size: u32,
    timeout: Duration,
}

impl QuickLookTemplatePreviewGenerator {
    #[must_use]
    pub fn new(
        executable_path: PathBuf,
        max_bytes: usize,
        max_dimension: u32,
        render_size: u32,
        timeout: Duration,
    ) -> Self {
        Self {
            executable_path,
            max_bytes,
            max_dimension,
            render_size,
            timeout,
        }
    }
}

impl TemplatePreviewGenerator for QuickLookTemplatePreviewGenerator {
    fn supports_independent_slides(&self) -> bool {
        false
    }
    fn generate(&self, source: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        self.try_generate(source, None).or(Ok(None))
    }
    fn generate_document_cancellable(
        &self,
        source: &[u8],
        _document: &Value,
        cancellation: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        self.try_generate(source, cancellation).or(Ok(None))
    }
}

impl QuickLookTemplatePreviewGenerator {
    fn try_generate(
        &self,
        source: &[u8],
        cancellation: Option<&RequestCancellation>,
    ) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        if cancellation.is_some_and(RequestCancellation::is_cancelled) {
            return Ok(None);
        }
        let working_directory = tempdir()?;
        let input_path = working_directory.path().join("upload.pptx");
        let output_path = working_directory.path().join("preview");
        fs::write(&input_path, source)?;
        fs::create_dir(&output_path)?;

        let mut child = PreviewProcess(
            Command::new(&self.executable_path)
                .args(["-t", "-s", &self.render_size.to_string(), "-o"])
                .arg(&output_path)
                .arg(&input_path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        );
        let exceeded_limit = Arc::new(AtomicBool::new(false));
        let stdout = drain_process_output(
            child.0.stdout.take().expect("stdout was piped"),
            Arc::clone(&exceeded_limit),
        )?;
        let stderr = drain_process_output(
            child.0.stderr.take().expect("stderr was piped"),
            Arc::clone(&exceeded_limit),
        )?;
        let deadline = Instant::now() + self.timeout;
        let succeeded = loop {
            if cancellation.is_some_and(RequestCancellation::is_cancelled)
                || exceeded_limit.load(Ordering::Acquire)
                || Instant::now() >= deadline
            {
                let _ = child.0.kill();
                let _ = child.0.wait();
                break false;
            }
            match child.0.try_wait() {
                Ok(Some(status)) => break status.success(),
                Ok(None) => {}
                Err(error) => {
                    let _ = child.0.kill();
                    let _ = child.0.wait();
                    return Err(error.into());
                }
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout_result = stdout.join();
        let stderr_result = stderr.join();
        if !succeeded
            || exceeded_limit.load(Ordering::Acquire)
            || !matches!(stdout_result, Ok(Ok(())))
            || !matches!(stderr_result, Ok(Ok(())))
        {
            return Ok(None);
        }

        let mut entries = fs::read_dir(&output_path)?.collect::<Result<Vec<_>, _>>()?;
        if entries.len() != 1 {
            return Ok(None);
        }
        let entry = entries
            .pop()
            .ok_or_else(|| anyhow::anyhow!("preview entry missing"))?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || !entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
            || usize::try_from(metadata.len()).map_or(true, |size| size > self.max_bytes)
        {
            return Ok(None);
        }
        let bytes = fs::read(entry.path())?;
        if bytes.len() > self.max_bytes {
            return Ok(None);
        }
        let Some((width, height)) = png_dimensions(&bytes) else {
            return Ok(None);
        };
        if width > self.max_dimension || height > self.max_dimension {
            return Ok(None);
        }
        Ok(Some(GeneratedTemplatePreview {
            bytes,
            width,
            height,
        }))
    }
}

#[cfg(all(test, unix))]
#[path = "../../../tests/unit/adapters/slide_templates/preview_tests.rs"]
mod tests;
