use std::{path::Path as FilePath, sync::Arc};

use axum::{
    extract::{rejection::QueryRejection, Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::app::http::error::{AppError, AppResult};

use super::service::{
    PptxTemplateImportMode, PptxTemplateImportResult, PptxTemplateUpload, TemplatePreviewPage,
    TemplateService, MAX_PPTX_TEMPLATE_IMPORT_BYTES,
};

#[derive(Clone)]
pub(super) struct TemplatesHttpState {
    pub templates: Arc<TemplateService>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TemplatePreviewQuery {
    #[serde(default = "default_page")]
    page: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PptxTemplateImportQuery {
    mode: PptxTemplateImportMode,
}

fn default_page() -> usize {
    1
}

pub(super) async fn list_template_previews_handler(
    State(state): State<TemplatesHttpState>,
    query: Result<Query<TemplatePreviewQuery>, QueryRejection>,
) -> AppResult<Json<TemplatePreviewPage>> {
    let Query(query) =
        query.map_err(|_| AppError::bad_request("page must be a positive integer"))?;
    if query.page == 0 {
        return Err(AppError::bad_request("page must be a positive integer"));
    }
    state
        .templates
        .list_previews(query.page)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn delete_template_handler(
    State(state): State<TemplatesHttpState>,
    Path(template_id): Path<String>,
) -> AppResult<StatusCode> {
    state
        .templates
        .delete(&template_id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub(super) async fn import_pptx_template_handler(
    State(state): State<TemplatesHttpState>,
    query: Result<Query<PptxTemplateImportQuery>, QueryRejection>,
    multipart: Multipart,
) -> AppResult<(StatusCode, Json<PptxTemplateImportResult>)> {
    let Query(query) = query.map_err(|_| {
        AppError::bad_request("mode must be supplied exactly once as single or batch")
    })?;
    let upload = collect_pptx_template_upload(multipart).await?;
    state
        .templates
        .import_pptx_template(upload, query.mode)
        .await
        .map(|result| (StatusCode::CREATED, Json(result)))
        .map_err(AppError::from)
}

async fn collect_pptx_template_upload(mut multipart: Multipart) -> AppResult<PptxTemplateUpload> {
    let mut upload = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("the template upload could not be read"))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name != "files" {
            if !field
                .bytes()
                .await
                .map_err(|_| AppError::bad_request("the template upload could not be read"))?
                .is_empty()
            {
                return Err(AppError::bad_request(
                    "only one files upload field is supported",
                ));
            }
            continue;
        }
        if upload.is_some() {
            return Err(AppError::bad_request(
                "exactly one template file is required",
            ));
        }
        let filename = field
            .file_name()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::bad_request("the template file must include a filename"))?;
        validate_pptx_template_filename(filename)?;
        let bytes = field
            .bytes()
            .await
            .map_err(|_| AppError::bad_request("the template upload could not be read"))?;
        if bytes.is_empty() {
            return Err(AppError::bad_request("the template file must not be empty"));
        }
        if bytes.len() > MAX_PPTX_TEMPLATE_IMPORT_BYTES {
            return Err(AppError::bad_request(
                "the template file exceeds the 25 MB limit",
            ));
        }
        upload = Some(PptxTemplateUpload {
            bytes: bytes.to_vec(),
        });
    }
    upload.ok_or_else(|| AppError::bad_request("exactly one template file is required"))
}

fn validate_pptx_template_filename(filename: &str) -> AppResult<()> {
    let is_path_like = filename.trim() != filename
        || filename.contains(['/', '\\'])
        || filename.chars().any(char::is_control)
        || matches!(filename, "." | "..");
    let has_pptx_extension = FilePath::new(filename)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pptx"));
    if is_path_like || !has_pptx_extension {
        return Err(AppError::bad_request(
            "the template filename must be a safe .pptx name",
        ));
    }
    Ok(())
}
