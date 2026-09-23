use std::{path::Path as FilePath, sync::Arc};

use axum::{
    body::Body,
    extract::{rejection::QueryRejection, Multipart, Path, Query, State},
    http::{
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
        HeaderValue, Response, StatusCode,
    },
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::{
    adapters::diligence_studio::templates::{MAX_TEMPLATE_ID_BYTES, MAX_TEMPLATE_PREVIEW_PAGES},
    app::http::error::{AppError, AppResult},
};

use super::service::{
    PptxTemplateImportMode, PptxTemplateImportResult, PptxTemplateUpload, TemplatePreviewPage,
    TemplateService, MAX_PPTX_TEMPLATE_IMPORT_BYTES, POWERPOINT_CONTENT_TYPE,
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
    if query.page == 0 || query.page > MAX_TEMPLATE_PREVIEW_PAGES {
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
    validate_template_id(&template_id)?;
    state
        .templates
        .delete(&template_id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(AppError::from)
}

pub(super) async fn get_template_handler(
    State(state): State<TemplatesHttpState>,
    Path(template_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    validate_template_id(&template_id)?;
    let document = state
        .templates
        .get(&template_id)
        .await
        .map_err(AppError::from)?;
    Ok((
        [(CACHE_CONTROL, HeaderValue::from_static("private, no-store"))],
        Json(document),
    ))
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

pub(super) async fn export_powerpoint_handler(
    State(state): State<TemplatesHttpState>,
    Json(document): Json<serde_json::Value>,
) -> AppResult<Response<Body>> {
    validate_presentation_document(&document)?;
    let export = state
        .templates
        .export_powerpoint(document)
        .await
        .map_err(AppError::from)?;
    let disposition =
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", export.file_name)).map_err(
            |_| AppError::internal("validated PowerPoint filename could not be encoded"),
        )?;
    let warning_count = HeaderValue::from_str(&export.warning_count.to_string())
        .map_err(|_| AppError::internal("PowerPoint warning count could not be encoded"))?;
    Response::builder()
        .status(StatusCode::OK)
        .header(CACHE_CONTROL, "private, no-store")
        .header(CONTENT_TYPE, POWERPOINT_CONTENT_TYPE)
        .header(CONTENT_DISPOSITION, disposition)
        .header("x-powerpoint-warning-count", warning_count)
        .body(Body::from(export.bytes))
        .map_err(|error| {
            AppError::internal(format!("PowerPoint response could not be built: {error}"))
        })
}

fn validate_template_id(template_id: &str) -> AppResult<()> {
    if template_id.trim().is_empty()
        || template_id.len() > MAX_TEMPLATE_ID_BYTES
        || template_id.chars().any(char::is_control)
    {
        return Err(AppError::bad_request("template ID is invalid"));
    }
    Ok(())
}

fn validate_presentation_document(document: &serde_json::Value) -> AppResult<()> {
    if !document
        .as_object()
        .and_then(|root| root.get("presentation"))
        .is_some_and(serde_json::Value::is_object)
    {
        return Err(AppError::bad_request(
            "presentation document is missing a presentation object",
        ));
    }
    Ok(())
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
