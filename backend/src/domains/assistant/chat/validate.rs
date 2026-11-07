//! HTTP-independent attachment decoding, also used to guard persisted document contents.
use crate::shared::office_zip::{validate_office_package, OfficeDocumentKind};
use std::path::Path;

use super::model::{ChatUpload, ChatUploadKind, RawUpload};

pub(super) fn prepare_attachment(
    filename: String,
    bytes: Vec<u8>,
    mime: String,
) -> Result<ChatUpload, &'static str> {
    validate_filename(&filename)?;
    validate_upload(RawUpload {
        filename,
        bytes,
        browser_mime: mime,
    })
}

pub(super) fn validate_filename(filename: &str) -> Result<(), &'static str> {
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
        return Err("file name is invalid");
    }
    Ok(())
}

pub(super) fn validate_upload(raw: RawUpload) -> Result<ChatUpload, &'static str> {
    let extension = Path::new(&raw.filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or("unsupported file type")?;
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
        "docx" if valid_office_zip(&raw.bytes, OfficeDocumentKind::Word) => (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ChatUploadKind::File,
        ),
        "pptx" if valid_office_zip(&raw.bytes, OfficeDocumentKind::PowerPoint) => (
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ChatUploadKind::File,
        ),
        "xlsx" if valid_office_zip(&raw.bytes, OfficeDocumentKind::Spreadsheet) => (
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ChatUploadKind::File,
        ),
        _ => return Err("file type or contents are not supported"),
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

fn validate_browser_mime(
    browser: &str,
    canonical: &str,
    extension: &str,
) -> Result<(), &'static str> {
    let browser = browser.trim().to_ascii_lowercase();
    let generic = browser.is_empty() || browser == "application/octet-stream";
    let alias = (extension == "jpg" || extension == "jpeg") && browser == "image/jpg"
        || extension == "md" && browser == "text/plain";
    if !generic && browser != canonical && !alias {
        return Err("file MIME type contradicts its contents");
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

fn valid_office_zip(bytes: &[u8], kind: OfficeDocumentKind) -> bool {
    validate_office_package(bytes, kind).is_ok()
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/assistant/chat/validate_tests.rs"]
mod tests;
