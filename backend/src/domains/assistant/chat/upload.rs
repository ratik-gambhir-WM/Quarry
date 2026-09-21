use std::{io::Read, path::Path};

use axum::extract::Multipart;
use zip::ZipArchive;

use crate::{
    app::http::error::{AppError, AppResult},
    shared::file_policy::{MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES},
};

use super::model::{
    parse_context, validate_model, validate_prompt, validate_system_instructions,
    PersistentQueryInput, QueryModelInput,
};

const MAX_FILES: usize = 20;
const MAX_ZIP_ENTRIES: usize = 2_048;
const MAX_ZIP_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ZIP_TOTAL_BYTES: u64 = 100 * 1024 * 1024;
const MAX_CONTENT_TYPES_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub struct ChatUpload {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub kind: ChatUploadKind,
    pub mime_type: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatUploadKind {
    File,
    Image,
}

struct RawUpload {
    browser_mime: String,
    bytes: Vec<u8>,
    filename: String,
}

pub async fn collect_query_upload(mut multipart: Multipart) -> AppResult<QueryModelInput> {
    let fields = collect_fields(&mut multipart, false).await?;
    Ok(QueryModelInput {
        context: parse_context(&required(fields.context, "context")?)?,
        files: fields.files,
        model: validate_model(fields.model)?,
        prompt: validate_prompt(required(fields.prompt, "prompt")?)?,
        system_instructions: validate_system_instructions(fields.system_instructions)?,
    })
}

pub async fn collect_persistent_query_upload(
    mut multipart: Multipart,
    thread_id: String,
) -> AppResult<PersistentQueryInput> {
    let fields = collect_fields(&mut multipart, true).await?;
    validate_identifier("threadId", &thread_id)?;
    let user_email = required(fields.user_email, "userEmail")?;
    if user_email.trim().is_empty() || user_email.chars().count() > 320 {
        return Err(AppError::bad_request("userEmail is invalid"));
    }
    let user_message_id = required(fields.user_message_id, "userMessageId")?;
    let assistant_message_id = required(fields.assistant_message_id, "assistantMessageId")?;
    let request_id = required(fields.request_id, "requestId")?;
    validate_identifier("userMessageId", &user_message_id)?;
    validate_identifier("assistantMessageId", &assistant_message_id)?;
    validate_identifier("requestId", &request_id)?;
    if let Some(parent) = fields.parent_message_id.as_deref() {
        validate_identifier("parentMessageId", parent)?;
    }
    Ok(PersistentQueryInput {
        assistant_message_id,
        files: fields.files,
        model: validate_model(fields.model)?,
        parent_message_id: fields.parent_message_id,
        prompt: validate_prompt(required(fields.prompt, "prompt")?)?,
        request_id,
        system_instructions: validate_system_instructions(fields.system_instructions)?,
        thread_id,
        user_email: user_email.trim().to_string(),
        user_message_id,
    })
}

struct CollectedFields {
    assistant_message_id: Option<String>,
    context: Option<String>,
    files: Vec<ChatUpload>,
    model: Option<String>,
    parent_message_id: Option<String>,
    prompt: Option<String>,
    request_id: Option<String>,
    system_instructions: Option<String>,
    user_email: Option<String>,
    user_message_id: Option<String>,
}

async fn collect_fields(multipart: &mut Multipart, persistent: bool) -> AppResult<CollectedFields> {
    let mut prompt = None;
    let mut context = None;
    let mut model = None;
    let mut system_instructions = None;
    let mut user_email = None;
    let mut user_message_id = None;
    let mut assistant_message_id = None;
    let mut parent_message_id = None;
    let mut request_id = None;
    let mut raw_files = Vec::new();
    let mut total_bytes = 0usize;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("failed to read multipart field"))?
    {
        let name = field
            .name()
            .ok_or_else(|| AppError::bad_request("multipart field name is required"))?
            .to_string();
        match name.as_str() {
            "prompt" => set_once(&mut prompt, field.text().await, "prompt")?,
            "context" => set_once(&mut context, field.text().await, "context")?,
            "model" => set_once(&mut model, field.text().await, "model")?,
            "systemInstructions" => set_once(
                &mut system_instructions,
                field.text().await,
                "systemInstructions",
            )?,
            "userEmail" if persistent => {
                set_once(&mut user_email, field.text().await, "userEmail")?
            }
            "userMessageId" if persistent => {
                set_once(&mut user_message_id, field.text().await, "userMessageId")?
            }
            "assistantMessageId" if persistent => set_once(
                &mut assistant_message_id,
                field.text().await,
                "assistantMessageId",
            )?,
            "parentMessageId" if persistent => set_once(
                &mut parent_message_id,
                field.text().await,
                "parentMessageId",
            )?,
            "requestId" if persistent => {
                set_once(&mut request_id, field.text().await, "requestId")?
            }
            "files" => {
                if raw_files.len() >= MAX_FILES {
                    return Err(AppError::bad_request("at most 20 files are allowed"));
                }
                let filename = field
                    .file_name()
                    .ok_or_else(|| AppError::bad_request("file name is required"))?
                    .to_string();
                validate_filename(&filename)?;
                let browser_mime = field.content_type().unwrap_or_default().to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::bad_request("failed to read uploaded file"))?
                    .to_vec();
                if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
                    return Err(AppError::bad_request("file is empty or exceeds 50 MB"));
                }
                total_bytes = total_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(|| AppError::bad_request("uploaded files are too large"))?;
                if total_bytes > MAX_TOTAL_REQUEST_FILE_BYTES {
                    return Err(AppError::bad_request(
                        "uploaded files exceed the 50 MB total request limit",
                    ));
                }
                raw_files.push(RawUpload {
                    browser_mime,
                    bytes,
                    filename,
                });
            }
            _ => return Err(AppError::bad_request("unknown multipart field")),
        }
    }

    if persistent && context.is_some() {
        return Err(AppError::bad_request(
            "context is not accepted for persisted threads",
        ));
    }
    let files = tokio::task::spawn_blocking(move || {
        raw_files
            .into_iter()
            .map(validate_upload)
            .collect::<AppResult<Vec<_>>>()
    })
    .await
    .map_err(|error| AppError::internal(format!("file validation worker failed: {error}")))??;

    Ok(CollectedFields {
        assistant_message_id,
        context,
        files,
        model,
        parent_message_id,
        prompt,
        request_id,
        system_instructions,
        user_email,
        user_message_id,
    })
}

fn validate_identifier(name: &str, value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::bad_request(format!("{name} is invalid")));
    }
    Ok(())
}

fn set_once(
    target: &mut Option<String>,
    value: Result<String, axum::extract::multipart::MultipartError>,
    name: &str,
) -> AppResult<()> {
    if target.is_some() {
        return Err(AppError::bad_request(format!("duplicate {name} field")));
    }
    *target = Some(value.map_err(|_| AppError::bad_request(format!("invalid {name} field")))?);
    Ok(())
}

fn required(value: Option<String>, name: &str) -> AppResult<String> {
    value.ok_or_else(|| AppError::bad_request(format!("{name} field is required")))
}

fn validate_filename(filename: &str) -> AppResult<()> {
    if filename.is_empty()
        || filename.len() > 255
        || filename.trim() != filename
        || filename.contains(['/', '\\'])
        || filename.chars().any(char::is_control)
        || Path::new(filename)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(filename)
    {
        return Err(AppError::bad_request("file name is invalid"));
    }
    Ok(())
}

fn validate_upload(raw: RawUpload) -> AppResult<ChatUpload> {
    let extension = Path::new(&raw.filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| AppError::bad_request("unsupported file type"))?;
    let (mime_type, kind) = match extension.as_str() {
        "png" if raw.bytes.starts_with(b"\x89PNG\r\n\x1a\n") => {
            ("image/png", ChatUploadKind::Image)
        }
        "jpg" | "jpeg" if raw.bytes.starts_with(&[0xff, 0xd8, 0xff]) => {
            ("image/jpeg", ChatUploadKind::Image)
        }
        "webp"
            if raw.bytes.len() >= 12
                && &raw.bytes[..4] == b"RIFF"
                && &raw.bytes[8..12] == b"WEBP" =>
        {
            ("image/webp", ChatUploadKind::Image)
        }
        "gif" if is_single_frame_gif(&raw.bytes) => ("image/gif", ChatUploadKind::Image),
        "pdf" if raw.bytes.starts_with(b"%PDF-") => ("application/pdf", ChatUploadKind::File),
        "txt" if valid_text(&raw.bytes) => ("text/plain", ChatUploadKind::File),
        "md" if valid_text(&raw.bytes) => ("text/markdown", ChatUploadKind::File),
        "json" if valid_text(&raw.bytes) => ("application/json", ChatUploadKind::File),
        "html" if valid_text(&raw.bytes) => ("text/html", ChatUploadKind::File),
        "csv" if valid_text(&raw.bytes) => ("text/csv", ChatUploadKind::File),
        "doc"
            if raw
                .bytes
                .starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) =>
        {
            ("application/msword", ChatUploadKind::File)
        }
        "docx" if valid_office_zip(&raw.bytes, OfficeFamily::Word)? => (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ChatUploadKind::File,
        ),
        "pptx" if valid_office_zip(&raw.bytes, OfficeFamily::PowerPoint)? => (
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ChatUploadKind::File,
        ),
        "xlsx" if valid_office_zip(&raw.bytes, OfficeFamily::Excel)? => (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ChatUploadKind::File,
        ),
        _ => {
            return Err(AppError::bad_request(
                "file type or contents are not supported",
            ))
        }
    };
    validate_browser_mime(&raw.browser_mime, mime_type, extension.as_str())?;
    Ok(ChatUpload {
        bytes: raw.bytes,
        filename: raw.filename,
        kind,
        mime_type,
    })
}

fn valid_text(bytes: &[u8]) -> bool {
    !bytes.is_empty() && std::str::from_utf8(bytes).is_ok()
}

fn validate_browser_mime(browser: &str, canonical: &str, extension: &str) -> AppResult<()> {
    let browser = browser.trim().to_ascii_lowercase();
    let generic = browser.is_empty() || browser == "application/octet-stream";
    let alias = (extension == "jpg" || extension == "jpeg") && browser == "image/jpg"
        || extension == "md" && browser == "text/plain";
    if !generic && browser != canonical && !alias {
        return Err(AppError::bad_request(
            "file MIME type contradicts its contents",
        ));
    }
    Ok(())
}

fn is_single_frame_gif(bytes: &[u8]) -> bool {
    if bytes.len() < 13 || (!bytes.starts_with(b"GIF87a") && !bytes.starts_with(b"GIF89a")) {
        return false;
    }
    let packed = bytes[10];
    let mut offset = 13usize;
    if packed & 0x80 != 0 {
        let table_size = 3usize << usize::from((packed & 0x07) + 1);
        offset = match offset.checked_add(table_size) {
            Some(value) => value,
            None => return false,
        };
    }
    let mut frames = 0usize;
    while offset < bytes.len() {
        match bytes[offset] {
            0x3b => return frames == 1,
            0x21 => {
                offset += 2;
                if !skip_sub_blocks(bytes, &mut offset) {
                    return false;
                }
            }
            0x2c => {
                frames += 1;
                if frames > 1 || offset + 10 > bytes.len() {
                    return false;
                }
                let local = bytes[offset + 9];
                offset += 10;
                if local & 0x80 != 0 {
                    let table_size = 3usize << usize::from((local & 0x07) + 1);
                    offset = match offset.checked_add(table_size) {
                        Some(value) => value,
                        None => return false,
                    };
                }
                if offset >= bytes.len() {
                    return false;
                }
                offset += 1;
                if !skip_sub_blocks(bytes, &mut offset) {
                    return false;
                }
            }
            _ => return false,
        }
    }
    false
}

fn skip_sub_blocks(bytes: &[u8], offset: &mut usize) -> bool {
    loop {
        let Some(&length) = bytes.get(*offset) else {
            return false;
        };
        *offset += 1;
        if length == 0 {
            return true;
        }
        *offset = match offset.checked_add(usize::from(length)) {
            Some(value) => value,
            None => return false,
        };
        if *offset > bytes.len() {
            return false;
        }
    }
}

enum OfficeFamily {
    Word,
    PowerPoint,
    Excel,
}

fn valid_office_zip(bytes: &[u8], family: OfficeFamily) -> AppResult<bool> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = match ZipArchive::new(cursor) {
        Ok(archive) => archive,
        Err(_) => return Ok(false),
    };
    if archive.len() > MAX_ZIP_ENTRIES {
        return Ok(false);
    }
    let expected = match family {
        OfficeFamily::Word => "word/document.xml",
        OfficeFamily::PowerPoint => "ppt/presentation.xml",
        OfficeFamily::Excel => "xl/workbook.xml",
    };
    let mut has_expected = false;
    let mut content_types = None;
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| AppError::bad_request("invalid Office archive"))?;
        if entry.size() > MAX_ZIP_ENTRY_BYTES {
            return Ok(false);
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| AppError::bad_request("Office archive is too large"))?;
        if total > MAX_ZIP_TOTAL_BYTES {
            return Ok(false);
        }
        if entry.name() == expected {
            has_expected = true;
        }
        if entry.name() == "[Content_Types].xml" {
            if entry.size() > MAX_CONTENT_TYPES_BYTES as u64 {
                return Ok(false);
            }
            let mut text = String::new();
            entry
                .take(MAX_CONTENT_TYPES_BYTES as u64 + 1)
                .read_to_string(&mut text)
                .map_err(|_| AppError::bad_request("invalid Office content types"))?;
            content_types = Some(text);
        }
    }
    let marker = match family {
        OfficeFamily::Word => "wordprocessingml.document",
        OfficeFamily::PowerPoint => "presentationml.presentation",
        OfficeFamily::Excel => "spreadsheetml.sheet",
    };
    Ok(has_expected && content_types.is_some_and(|types| types.contains(marker)))
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/assistant/chat/upload_tests.rs"]
mod tests;
