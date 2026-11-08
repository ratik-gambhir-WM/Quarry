use std::path::Path;

use axum::extract::Multipart;

use crate::{
    app::http::error::{AppError, AppResult},
    domains::research::service::WmUploadedFile,
    shared::file_policy::{
        infer_supported_mime_type, MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES,
    },
};

pub(crate) async fn collect_wm_upload_files(
    mut multipart: Multipart,
) -> AppResult<Vec<WmUploadedFile>> {
    let mut files = Vec::new();
    let mut skipped_files = Vec::new();
    let mut total_file_bytes = 0usize;
    let mut total_limit_reached = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| AppError::bad_request(format!("failed to read multipart field: {err}")))?
    {
        if field.name() != Some("files") {
            continue;
        }

        let filename = field.file_name().unwrap_or("upload").to_string();
        let Some(mime_type) = infer_supported_mime_type(Path::new(&filename)) else {
            skipped_files.push(filename);
            continue;
        };

        if total_limit_reached {
            skipped_files.push(format!(
                "{filename} (skipped: total request file size limit already reached)"
            ));
            continue;
        }

        let bytes = field.bytes().await.map_err(|err| {
            AppError::bad_request(format!("failed to read upload {filename}: {err}"))
        })?;
        let file_size_bytes = bytes.len();

        if file_size_bytes == 0 {
            skipped_files.push(format!("{filename} (empty)"));
            continue;
        }
        if file_size_bytes > MAX_FILE_BYTES {
            skipped_files.push(format!("{filename} (skipped: file exceeds 50 MB limit)"));
            continue;
        }
        if total_file_bytes + file_size_bytes > MAX_TOTAL_REQUEST_FILE_BYTES {
            skipped_files.push(format!(
                "{filename} (skipped: total request file size would exceed 50 MB limit)"
            ));
            total_limit_reached = true;
            continue;
        }

        files.push(WmUploadedFile {
            filename: Path::new(&filename)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&filename)
                .to_string(),
            relative_path: filename,
            mime_type,
            bytes: bytes.to_vec(),
        });
        total_file_bytes += file_size_bytes;
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    skipped_files.sort();

    if files.is_empty() {
        if skipped_files.is_empty() {
            return Err(AppError::bad_request(
                "at least one file upload is required",
            ));
        }
        return Err(AppError::bad_request(format!(
            "no supported files found; skipped: {}",
            skipped_files.join(", ")
        )));
    }

    Ok(files)
}
