use std::{collections::HashSet, io::Cursor, time::Duration};

use base64::{engine::general_purpose, Engine as _};
use futures_util::StreamExt;
use image::{io::Reader as ImageReader, ImageFormat};
use reqwest::{header::CONTENT_TYPE, Response, StatusCode};
use serde::Deserialize;
use thiserror::Error;

use super::client::{DiligenceStudioClient, APP_ID_HEADER, DILIGENCE_STUDIO_APP_ID};

pub const MAX_TEMPLATE_CATALOG_ITEMS: usize = 1_000;
pub const MAX_TEMPLATE_PREVIEW_PAGES: usize = 100;
const MAX_PREVIEWS_PER_PAGE: usize = 10;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_IMPORT_ERROR_BYTES: usize = 16 * 1024;
const MAX_IMPORTED_TEMPLATE_COUNT: usize = 1_000;
const MAX_IMPORT_WARNING_COUNT: usize = 10_000;
const MAX_PREVIEW_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIMENSION: u32 = 8_192;
const MAX_PAGE_PIXELS: u64 = 100_000_000;
pub const MAX_TEMPLATE_ID_BYTES: usize = 256;
const MAX_PREVIEW_URL_BYTES: usize = 2_048;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const PNG_DATA_URL_PREFIX: &str = "data:image/png;base64,";
pub const POWERPOINT_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PptxTemplateImportMode {
    Single,
    Batch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PptxTemplateImportResult {
    pub imported_count: usize,
    pub warning_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplatePreviewPage {
    pub pagination: TemplatePreviewPagination,
    pub previews: Vec<TemplatePreview>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplatePreviewPagination {
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
    pub has_next_page: bool,
    pub has_previous_page: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplatePreview {
    pub template_id: String,
    pub content_type: String,
    pub data_url: String,
    pub preview_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Error)]
pub enum SlideTemplateClientError {
    #[error("upstream request could not be created: {0}")]
    RequestBuild(String),
    #[error("upstream request failed: {0}")]
    Request(String),
    #[error("upstream returned status {0}")]
    Status(StatusCode),
    #[error("the slide-template import contained more than one slide")]
    PptxTemplateMustHaveOneSlide,
    #[error("upstream response exceeded its byte limit")]
    ResponseTooLarge,
    #[error("upstream response was not valid JSON: {0}")]
    InvalidJson(String),
    #[error("upstream template payload is invalid: {0}")]
    InvalidPayload(String),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamPage {
    pagination: UpstreamPagination,
    previews: Vec<UpstreamPreview>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamPagination {
    page: usize,
    page_size: usize,
    total_items: usize,
    total_pages: usize,
    has_next_page: bool,
    has_previous_page: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpstreamPreview {
    template_id: String,
    content_type: String,
    data_url: String,
    preview_url: String,
    width: u32,
    height: u32,
}

impl DiligenceStudioClient {
    pub async fn list_template_previews(
        &self,
        requested_page: usize,
    ) -> Result<TemplatePreviewPage, SlideTemplateClientError> {
        let mut endpoint = self
            .endpoint("templates/previews")
            .map_err(SlideTemplateClientError::RequestBuild)?;
        endpoint
            .query_pairs_mut()
            .append_pair("page", &requested_page.to_string());
        let response = self
            .http
            .get(endpoint)
            .header(APP_ID_HEADER, DILIGENCE_STUDIO_APP_ID)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|error| SlideTemplateClientError::Request(error.to_string()))?;
        if !response.status().is_success() {
            return Err(SlideTemplateClientError::Status(response.status()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(SlideTemplateClientError::ResponseTooLarge);
        }

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|error| SlideTemplateClientError::Request(error.to_string()))?;
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(SlideTemplateClientError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }

        let payload = serde_json::from_slice::<UpstreamPage>(&bytes)
            .map_err(|error| SlideTemplateClientError::InvalidJson(error.to_string()))?;
        validate_page(payload, requested_page)
    }

    pub async fn delete_template(&self, template_id: &str) -> Result<(), SlideTemplateClientError> {
        if !valid_template_id(template_id) {
            return invalid("template ID is invalid");
        }
        let mut endpoint = self
            .endpoint("templates/")
            .map_err(SlideTemplateClientError::RequestBuild)?;
        endpoint
            .path_segments_mut()
            .map_err(|_| {
                SlideTemplateClientError::RequestBuild(
                    "Diligence Studio base URL cannot accept path segments".to_string(),
                )
            })?
            .pop_if_empty()
            .push(template_id);
        let response = self
            .http
            .delete(endpoint)
            .header(APP_ID_HEADER, DILIGENCE_STUDIO_APP_ID)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|error| SlideTemplateClientError::Request(error.to_string()))?;
        if response.status() == StatusCode::NO_CONTENT {
            return Ok(());
        }
        if !response.status().is_success() {
            return Err(SlideTemplateClientError::Status(response.status()));
        }
        invalid("delete response was not 204 No Content")
    }

    pub async fn import_pptx_template(
        &self,
        bytes: Vec<u8>,
        mode: PptxTemplateImportMode,
    ) -> Result<PptxTemplateImportResult, SlideTemplateClientError> {
        let path = match mode {
            PptxTemplateImportMode::Single => "import",
            PptxTemplateImportMode::Batch => "batchImport",
        };
        let mut endpoint = self
            .endpoint(path)
            .map_err(SlideTemplateClientError::RequestBuild)?;
        endpoint.query_pairs_mut().append_pair("kind", "diagram");
        let response = self
            .http
            .post(endpoint)
            .header(APP_ID_HEADER, DILIGENCE_STUDIO_APP_ID)
            .header(CONTENT_TYPE, POWERPOINT_CONTENT_TYPE)
            .timeout(REQUEST_TIMEOUT)
            .body(bytes)
            .send()
            .await
            .map_err(|error| {
                SlideTemplateClientError::Request(if error.is_timeout() {
                    "request timed out".to_string()
                } else {
                    "request transport failed".to_string()
                })
            })?;

        if response.status() != StatusCode::CREATED {
            let status = response.status();
            let code = read_bounded_error_code(response).await;
            if mode == PptxTemplateImportMode::Single
                && status == StatusCode::UNPROCESSABLE_ENTITY
                && code.as_deref() == Some("template_must_have_one_slide")
            {
                return Err(SlideTemplateClientError::PptxTemplateMustHaveOneSlide);
            }
            return Err(SlideTemplateClientError::Status(status));
        }
        if response_content_type(&response) != Some("application/json") {
            return invalid("import response content type is not application/json");
        }

        validate_pptx_import_headers(response.headers(), mode)
    }
}

fn validate_pptx_import_headers(
    headers: &reqwest::header::HeaderMap,
    mode: PptxTemplateImportMode,
) -> Result<PptxTemplateImportResult, SlideTemplateClientError> {
    let warning_count = parse_count_header(
        headers,
        "x-powerpoint-warning-count",
        true,
        MAX_IMPORT_WARNING_COUNT,
    )?;
    match mode {
        PptxTemplateImportMode::Single => {
            let template_id = required_header(headers, "x-template-id")?;
            if !valid_template_id(template_id) {
                return invalid("import response template ID is invalid");
            }
            if !matches!(
                required_header(headers, "x-template-preview-status")?,
                "ready" | "unavailable"
            ) {
                return invalid("import response preview status is invalid");
            }
            if headers.contains_key("x-imported-template-count") {
                return invalid("single import response contained batch headers");
            }
            Ok(PptxTemplateImportResult {
                imported_count: 1,
                warning_count,
            })
        }
        PptxTemplateImportMode::Batch => {
            if headers.contains_key("x-template-id")
                || headers.contains_key("x-template-preview-status")
            {
                return invalid("batch import response contained single-template headers");
            }
            Ok(PptxTemplateImportResult {
                imported_count: parse_count_header(
                    headers,
                    "x-imported-template-count",
                    false,
                    MAX_IMPORTED_TEMPLATE_COUNT,
                )?,
                warning_count,
            })
        }
    }
}

fn parse_count_header(
    headers: &reqwest::header::HeaderMap,
    name: &str,
    allow_zero: bool,
    maximum: usize,
) -> Result<usize, SlideTemplateClientError> {
    let value = required_header(headers, name)?
        .parse::<usize>()
        .map_err(|_| invalid_error("import response count header is invalid"))?;
    if (!allow_zero && value == 0) || value > maximum {
        return invalid("import response count header is outside the supported range");
    }
    Ok(value)
}

fn required_header<'a>(
    headers: &'a reqwest::header::HeaderMap,
    name: &str,
) -> Result<&'a str, SlideTemplateClientError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid_error("import response is missing a required header"))
}

fn response_content_type(response: &Response) -> Option<&str> {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
}

async fn read_bounded_error_code(response: Response) -> Option<String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_IMPORT_ERROR_BYTES as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        if bytes.len().saturating_add(chunk.len()) > MAX_IMPORT_ERROR_BYTES {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()?
        .get("error")?
        .get("code")?
        .as_str()
        .map(str::to_string)
}

fn validate_page(
    payload: UpstreamPage,
    requested_page: usize,
) -> Result<TemplatePreviewPage, SlideTemplateClientError> {
    let pagination = &payload.pagination;
    let empty_catalog = pagination.page == 1
        && pagination.page_size > 0
        && pagination.total_items == 0
        && pagination.total_pages == 0
        && !pagination.has_next_page
        && !pagination.has_previous_page
        && payload.previews.is_empty();
    let expected_pages = pagination
        .total_items
        .checked_add(pagination.page_size.saturating_sub(1))
        .filter(|_| pagination.page_size > 0)
        .map(|items| items / pagination.page_size);
    let pagination_valid = pagination.page == requested_page
        && pagination.page > 0
        && (1..=MAX_PREVIEWS_PER_PAGE).contains(&pagination.page_size)
        && pagination.total_items <= MAX_TEMPLATE_CATALOG_ITEMS
        && pagination.total_pages <= MAX_TEMPLATE_PREVIEW_PAGES
        && payload.previews.len() <= pagination.page_size
        && payload.previews.len() <= MAX_PREVIEWS_PER_PAGE
        && (empty_catalog
            || (pagination.total_items > 0
                && expected_pages == Some(pagination.total_pages)
                && pagination.page <= pagination.total_pages
                && pagination.has_previous_page == (pagination.page > 1)
                && pagination.has_next_page == (pagination.page < pagination.total_pages)));
    if !pagination_valid {
        return Err(invalid_error("pagination metadata is inconsistent"));
    }

    let mut total_pixels = 0u64;
    let mut template_ids = HashSet::new();
    let previews = payload
        .previews
        .into_iter()
        .map(|preview| {
            if !template_ids.insert(preview.template_id.clone()) {
                return invalid("template IDs must be unique within a page");
            }
            validate_preview(&preview, &mut total_pixels)?;
            Ok(TemplatePreview {
                template_id: preview.template_id,
                content_type: preview.content_type,
                data_url: preview.data_url,
                preview_url: preview.preview_url,
                width: preview.width,
                height: preview.height,
            })
        })
        .collect::<Result<Vec<_>, SlideTemplateClientError>>()?;

    Ok(TemplatePreviewPage {
        pagination: TemplatePreviewPagination {
            page: pagination.page,
            page_size: pagination.page_size,
            total_items: pagination.total_items,
            total_pages: pagination.total_pages,
            has_next_page: pagination.has_next_page,
            has_previous_page: pagination.has_previous_page,
        },
        previews,
    })
}

fn validate_preview(
    preview: &UpstreamPreview,
    total_pixels: &mut u64,
) -> Result<(), SlideTemplateClientError> {
    if !valid_template_id(&preview.template_id) {
        return invalid("template ID is invalid");
    }
    if preview.content_type != "image/png" {
        return invalid("content type is not image/png");
    }
    if preview.width == 0
        || preview.height == 0
        || preview.width > MAX_DIMENSION
        || preview.height > MAX_DIMENSION
    {
        return invalid("preview dimensions are invalid");
    }
    *total_pixels = total_pixels
        .checked_add(u64::from(preview.width) * u64::from(preview.height))
        .ok_or_else(|| invalid_error("preview pixel count overflowed"))?;
    if *total_pixels > MAX_PAGE_PIXELS {
        return invalid("preview pixel budget was exceeded");
    }
    validate_preview_url(&preview.preview_url)?;
    let encoded = preview
        .data_url
        .strip_prefix(PNG_DATA_URL_PREFIX)
        .ok_or_else(|| invalid_error("preview data URL is not a PNG"))?;
    if encoded.is_empty() || encoded.len() > (MAX_PREVIEW_BYTES * 4 / 3) + 4 {
        return invalid("preview image exceeded its byte limit");
    }
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid_error("preview image is not valid base64"))?;
    if decoded.is_empty()
        || decoded.len() > MAX_PREVIEW_BYTES
        || !decoded.starts_with(b"\x89PNG\r\n\x1a\n")
    {
        return invalid("preview image is not valid PNG data");
    }
    let dimensions = ImageReader::with_format(Cursor::new(&decoded), ImageFormat::Png)
        .into_dimensions()
        .map_err(|_| invalid_error("preview image is not a readable PNG"))?;
    if dimensions != (preview.width, preview.height) {
        return invalid("preview dimensions do not match the PNG");
    }
    Ok(())
}

fn validate_preview_url(value: &str) -> Result<(), SlideTemplateClientError> {
    if value.is_empty()
        || value.len() > MAX_PREVIEW_URL_BYTES
        || value.starts_with("//")
        || value.starts_with('\\')
        || value.contains(['#', '\r', '\n'])
    {
        return invalid("preview URL is unsafe");
    }
    let base = reqwest::Url::parse("https://preview.invalid/").expect("static URL is valid");
    let resolved = base
        .join(value)
        .map_err(|_| invalid_error("preview URL is unsafe"))?;
    if resolved.origin() != base.origin()
        || !resolved.username().is_empty()
        || resolved.password().is_some()
        || resolved.fragment().is_some()
    {
        return invalid("preview URL is unsafe");
    }
    let query = resolved.query_pairs().collect::<Vec<_>>();
    if query.len() != 1 || query[0].0 != "appId" || query[0].1 != DILIGENCE_STUDIO_APP_ID {
        return invalid("preview URL is not scoped to the Quarry app");
    }
    Ok(())
}

fn valid_template_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEMPLATE_ID_BYTES
        && !value.chars().any(char::is_control)
}

fn invalid<T>(message: &str) -> Result<T, SlideTemplateClientError> {
    Err(invalid_error(message))
}

fn invalid_error(message: &str) -> SlideTemplateClientError {
    SlideTemplateClientError::InvalidPayload(message.to_string())
}

#[cfg(test)]
#[path = "../../../tests/unit/adapters/diligence_studio/templates_tests.rs"]
mod tests;
