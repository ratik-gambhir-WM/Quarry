//! Native HTTP mappings; Quarry supplies request IDs, timeout, tracing and CORS.
use axum::{
    body::{to_bytes, Body},
    extract::{Multipart, Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use super::{error::FailureKind, *};
#[derive(Clone)]
pub(super) struct EndpointState {
    pub studio: Arc<SlideTemplates>,
}
impl IntoResponse for SlideTemplateError {
    fn into_response(self) -> Response {
        let status = match self.kind {
            FailureKind::Invalid => StatusCode::BAD_REQUEST,
            FailureKind::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            FailureKind::Unsupported => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            FailureKind::NotFound => StatusCode::NOT_FOUND,
            FailureKind::Unavailable | FailureKind::Cancelled => StatusCode::SERVICE_UNAVAILABLE,
            FailureKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            FailureKind::Timeout => StatusCode::GATEWAY_TIMEOUT,
            FailureKind::Upstream => StatusCode::BAD_GATEWAY,
        };
        if self.kind == FailureKind::Internal {
            tracing::error!(code=%self.code,"slide template operation failed");
        }
        (status, Json(serde_json::json!({"error":self.message}))).into_response()
    }
}
fn invalid(message: &'static str) -> SlideTemplateError {
    SlideTemplateError::new(FailureKind::Invalid, "invalid_request", message)
}
fn absent() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::NotFound,
        "template_not_found",
        "Template was not found.",
    )
}
fn validate_id(id: &str) -> SlideResult<()> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return Err(invalid("Template ID is invalid."));
    }
    Ok(())
}
fn query(
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    allowed: &[&str],
) -> SlideResult<Vec<(String, String)>> {
    let pairs = url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .into_owned()
        .collect::<Vec<_>>();
    for (index, (key, value)) in pairs.iter().enumerate() {
        if key == "appId" {
            if value != super::APP_ID {
                return Err(invalid("The application scope is fixed by the server."));
            }
        } else if !allowed.contains(&key.as_str()) {
            return Err(invalid("The query contains an unsupported parameter."));
        }
        if pairs[..index].iter().any(|(k, _)| k == key) {
            return Err(invalid("Query parameters must not be repeated."));
        }
    }
    let app_headers = headers.get_all("x-app-id").iter().collect::<Vec<_>>();
    if app_headers.len() > 1
        || app_headers
            .first()
            .is_some_and(|h| h.to_str().ok() != Some(super::APP_ID))
    {
        return Err(invalid("The application scope is fixed by the server."));
    }
    Ok(pairs)
}
fn kind(pairs: &[(String, String)]) -> SlideResult<TemplateKind> {
    match pairs
        .iter()
        .find(|(k, _)| k == "kind")
        .map(|(_, v)| v.as_str())
        .unwrap_or("diagram")
    {
        "diagram" => Ok(TemplateKind::Diagram),
        "commentary" => Ok(TemplateKind::Commentary),
        _ => Err(invalid("Template kind must be diagram or commentary.")),
    }
}
fn headers(headers: &HeaderMap, mime: &str, limit: usize) -> SlideResult<()> {
    if headers.get_all(header::CONTENT_TYPE).iter().count() != 1
        || headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(str::trim)
            != Some(mime)
    {
        return Err(SlideTemplateError::new(
            FailureKind::Unsupported,
            "unsupported_media_type",
            "The request content type is not supported.",
        ));
    }
    if headers
        .get(header::CONTENT_ENCODING)
        .is_some_and(|h| h.as_bytes() != b"identity")
    {
        return Err(SlideTemplateError::new(
            FailureKind::Unsupported,
            "unsupported_encoding",
            "Compressed request bodies are not supported.",
        ));
    }
    if let Some(size) = headers.get(header::CONTENT_LENGTH) {
        let size = size
            .to_str()
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or_else(|| invalid("Content length is invalid."))?;
        if size > limit as u64 {
            return Err(too_large());
        }
    }
    Ok(())
}
fn too_large() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::TooLarge,
        "payload_too_large",
        "The request exceeds the processing limit.",
    )
}
async fn bytes(request: Request, limit: usize) -> SlideResult<Vec<u8>> {
    to_bytes(request.into_body(), limit)
        .await
        .map(|b| b.to_vec())
        .map_err(|_| too_large())
}
fn private(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("private, no-store"),
    );
    response
}
fn header_value(value: &str) -> SlideResult<header::HeaderValue> {
    header::HeaderValue::from_str(value).map_err(SlideTemplateError::internal)
}
fn powerpoint(result: ExportedPowerPoint) -> SlideResult<Response> {
    if result.bytes.len() > super::super::model::MAX_POWERPOINT_EXPORT_BYTES
        || !result.bytes.starts_with(b"PK\x03\x04")
    {
        return Err(too_large());
    }
    let mut response = Body::from(result.bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header_value(super::super::model::POWERPOINT_CONTENT_TYPE)?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        header_value(&format!("attachment; filename=\"{}\"", result.file_name))?,
    );
    response.headers_mut().insert(
        "x-powerpoint-warning-count",
        header_value(&result.warnings.len().to_string())?,
    );
    Ok(private(response))
}
pub(super) async fn export(
    State(s): State<EndpointState>,
    request: Request,
) -> SlideResult<Response> {
    query(request.uri(), request.headers(), &[])?;
    headers(
        request.headers(),
        "application/json",
        super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES,
    )?;
    let input = s
        .studio
        .parse_json(bytes(request, super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES).await?)
        .await?;
    powerpoint(s.studio.export(input).await?)
}
pub(super) async fn import(
    State(s): State<EndpointState>,
    request: Request,
) -> SlideResult<Response> {
    import_inner(s, request, false, false).await
}
pub(super) async fn batch_import(
    State(s): State<EndpointState>,
    request: Request,
) -> SlideResult<Response> {
    import_inner(s, request, true, false).await
}
pub(super) async fn import_classified(
    State(s): State<EndpointState>,
    request: Request,
) -> SlideResult<Response> {
    import_inner(s, request, false, true).await
}
async fn import_inner(
    s: EndpointState,
    request: Request,
    batch: bool,
    classified: bool,
) -> SlideResult<Response> {
    let pairs = query(request.uri(), request.headers(), &["kind"])?;
    let kind = kind(&pairs)?;
    headers(
        request.headers(),
        super::super::model::POWERPOINT_CONTENT_TYPE,
        super::super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
    )?;
    let source = bytes(request, super::super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES).await?;
    if source.is_empty() {
        return Err(invalid("A PowerPoint file is required."));
    }
    if batch {
        let result = s.studio.batch(source, kind).await?;
        let count = result.templates.len();
        let warnings = result.warnings.len();
        let mut response = json_response(&s.studio, StatusCode::CREATED, move || {
            let mut body = serde_json::to_value(&result).map_err(SlideTemplateError::internal)?;
            if let Some(templates) = body["templates"].as_array_mut() {
                for (value, template) in templates.iter_mut().zip(&result.templates) {
                    value["templateJson"] = compact_document(&template.template_json);
                }
            }
            Ok(body)
        })
        .await?;
        response.headers_mut().insert(
            "x-imported-template-count",
            header_value(&count.to_string())?,
        );
        response.headers_mut().insert(
            "x-powerpoint-warning-count",
            header_value(&warnings.to_string())?,
        );
        return Ok(private(response));
    }
    let result = if classified {
        s.studio.classified(source, kind).await?
    } else {
        s.studio.import(source, kind).await?
    };
    let template_id = result.template_id;
    let preview_status = result.preview_status;
    let warning_count = result.warnings.len();
    let response_id = template_id.clone();
    let mut response=json_response(&s.studio,StatusCode::CREATED,move||Ok(if classified {serde_json::json!({"templateId":response_id,"templateJson":compact_document(&result.template_json),"warnings":result.warnings,"previewAvailable":preview_status==TemplatePreviewStatus::Ready,"retrieval":{"status":"ready"}})}else{compact_document(&result.template_json)})).await?;
    let location = format!(
        "{}/catalog/{}?appId=Quarry_WestMonroe",
        super::route::PREFIX,
        template_id
    );
    response
        .headers_mut()
        .insert(header::LOCATION, header_value(&location)?);
    response
        .headers_mut()
        .insert("x-template-id", header_value(&template_id)?);
    response.headers_mut().insert(
        "x-template-preview-status",
        header_value(preview_status.as_str())?,
    );
    response.headers_mut().insert(
        "x-powerpoint-warning-count",
        header_value(&warning_count.to_string())?,
    );
    if preview_status == TemplatePreviewStatus::Ready {
        response.headers_mut().insert(
            header::LINK,
            header_value(&format!(
                "<{}/catalog/{}/preview?appId=Quarry_WestMonroe>; rel=\"preview\"",
                super::route::PREFIX,
                template_id
            ))?,
        );
    }
    Ok(private(response))
}
pub(super) async fn catalog(
    State(s): State<EndpointState>,
    request: Request,
) -> SlideResult<Response> {
    let pairs = query(request.uri(), request.headers(), &["kind"])?;
    let filter = if pairs.iter().any(|(k, _)| k == "kind") {
        Some(kind(&pairs)?)
    } else {
        None
    };
    let result = s.studio.catalog(filter).await?;
    Ok(private(
        json_response(&s.studio, StatusCode::OK, move || Ok(result)).await?,
    ))
}
pub(super) async fn previews(
    State(s): State<EndpointState>,
    request: Request,
) -> SlideResult<Response> {
    let pairs = query(request.uri(), request.headers(), &["page"])?;
    let page = pairs
        .iter()
        .find(|(k, _)| k == "page")
        .map(|(_, v)| v.parse::<usize>())
        .transpose()
        .map_err(|_| invalid("Preview page must be a positive integer."))?
        .unwrap_or(1);
    if !(1..=super::super::model::MAX_TEMPLATE_PREVIEW_PAGES).contains(&page) {
        return Err(invalid("Preview page must be a positive integer."));
    }
    let result = s.studio.previews(page).await?;
    Ok(private(
        json_response(&s.studio, StatusCode::OK, move || Ok(result)).await?,
    ))
}
pub(super) async fn get(
    State(s): State<EndpointState>,
    Path(id): Path<String>,
    request: Request,
) -> SlideResult<Response> {
    query(request.uri(), request.headers(), &[])?;
    validate_id(&id)?;
    let result = s.studio.get(id).await?.ok_or_else(absent)?;
    Ok(private(
        json_response(&s.studio, StatusCode::OK, move || Ok(result)).await?,
    ))
}
pub(super) async fn delete(
    State(s): State<EndpointState>,
    Path(id): Path<String>,
    request: Request,
) -> SlideResult<Response> {
    query(request.uri(), request.headers(), &[])?;
    validate_id(&id)?;
    if !s.studio.delete(id).await? {
        return Err(absent());
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}
pub(super) async fn preview(
    State(s): State<EndpointState>,
    Path(id): Path<String>,
    request: Request,
) -> SlideResult<Response> {
    query(request.uri(), request.headers(), &[])?;
    validate_id(&id)?;
    let preview = s.studio.preview(id.clone()).await?;
    let Some(p) = preview else {
        if s.studio.get(id).await?.is_none() {
            return Err(absent());
        }
        return Err(super::facade::unavailable(
            "preview_unavailable",
            "The template preview is unavailable.",
        ));
    };
    if p.bytes.len() > 16 * 1024 * 1024 {
        return Err(too_large());
    }
    let mut response = Body::from(p.bytes).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, header_value("image/png")?);
    response.headers_mut().insert(
        "x-template-preview-width",
        header_value(&p.width.to_string())?,
    );
    response.headers_mut().insert(
        "x-template-preview-height",
        header_value(&p.height.to_string())?,
    );
    Ok(private(response))
}
pub(super) async fn asset(
    State(s): State<EndpointState>,
    Path((id, asset)): Path<(String, String)>,
    request: Request,
) -> SlideResult<Response> {
    query(request.uri(), request.headers(), &[])?;
    validate_id(&id)?;
    validate_id(&asset)?;
    let asset = s.studio.asset(id, asset).await?.ok_or_else(absent)?;
    if asset.bytes.len() > 40 * 1024 * 1024
        || !matches!(
            asset.content_type.as_str(),
            "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/svg+xml" | "image/emf"
        )
    {
        return Err(too_large());
    }
    let mut response = Body::from(asset.bytes).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, header_value(&asset.content_type)?);
    response
        .headers_mut()
        .insert("x-content-type-options", header_value("nosniff")?);
    if asset.content_type == "image/svg+xml" {
        // SVG images can become active documents on direct navigation. Keep their
        // origin opaque and block scripts, nested documents, and network access.
        response.headers_mut().insert(
            header::CONTENT_SECURITY_POLICY,
            header_value("sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'")?,
        );
    }
    Ok(private(response))
}
pub(super) async fn retrieval(
    State(s): State<EndpointState>,
    Path(id): Path<String>,
    request: Request,
) -> SlideResult<Response> {
    validate_id(&id)?;
    let pairs = query(
        request.uri(),
        request.headers(),
        &["metadata", "embeddings"],
    )?;
    let flag = |name: &str| -> SlideResult<bool> {
        match pairs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
        {
            None | Some("false") => Ok(false),
            Some("true") => Ok(true),
            _ => Err(invalid("Retrieval flags must be true or false.")),
        }
    };
    let result = s
        .studio
        .inspect(id, flag("metadata")?, flag("embeddings")?)
        .await?
        .ok_or_else(absent)?;
    Ok(private(
        json_response(&s.studio, StatusCode::OK, move || Ok(result)).await?,
    ))
}
pub(super) async fn insert(
    State(s): State<EndpointState>,
    request: Request,
) -> SlideResult<Response> {
    query(request.uri(), request.headers(), &[])?;
    headers(request.headers(), "multipart/form-data", 76 * 1024 * 1024)?;
    let (parts, body) = request.into_parts();
    let request = Request::from_parts(parts, body);
    let mut multipart = Multipart::from_request(request, &())
        .await
        .map_err(|_| invalid("A multipart PowerPoint insertion request is required."))?;
    let (mut presentation, mut target, mut after) = (None, None, None);
    let mut file_name = String::from("presentation.pptx");
    let mut count = 0;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| invalid("Invalid multipart request."))?
    {
        count += 1;
        if count > 3 {
            return Err(invalid("Unexpected multipart part."));
        }
        let name = field.name().unwrap_or_default().to_owned();
        let limit = match name.as_str() {
            "presentation" if presentation.is_none() => {
                super::super::model::MAX_TEMPLATE_DOCUMENT_BYTES
            }
            "target" if target.is_none() => super::super::model::MAX_PPTX_TEMPLATE_IMPORT_BYTES,
            "insertAfterSlide" if after.is_none() => 32,
            _ => return Err(invalid("Unexpected or duplicate multipart field.")),
        };
        if name == "target" {
            let file = field
                .file_name()
                .ok_or_else(|| invalid("The target must be a PowerPoint file."))?;
            if file.len() > 200
                || file.contains(['/', '\\', '\r', '\n', '"'])
                || !file.to_ascii_lowercase().ends_with(".pptx")
                || field.content_type() != Some(super::super::model::POWERPOINT_CONTENT_TYPE)
            {
                return Err(invalid("The target must be a safe .pptx file."));
            }
            file_name = file.to_owned();
        } else if field.file_name().is_some() {
            return Err(invalid("Unexpected file part."));
        }
        let mut data = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|_| invalid("Invalid multipart field."))?
        {
            if data.len().saturating_add(chunk.len()) > limit {
                return Err(too_large());
            }
            data.extend_from_slice(&chunk);
        }
        match name.as_str() {
            "presentation" => presentation = Some(s.studio.parse_json(data).await?),
            "target" => target = Some(data),
            _ => {
                let value = std::str::from_utf8(&data)
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|v| *v <= 9_007_199_254_740_991)
                    .ok_or_else(|| invalid("insertAfterSlide must be a non-negative integer."))?;
                after = Some(value);
            }
        }
    }
    powerpoint(
        s.studio
            .insert(InsertPowerPointInput {
                presentation: presentation.ok_or_else(|| invalid("Missing presentation."))?,
                target_bytes: target
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| invalid("Missing target PowerPoint."))?,
                target_file_name: file_name,
                insert_after_slide: after.ok_or_else(|| invalid("Missing insertAfterSlide."))?,
            })
            .await?,
    )
}
use axum::extract::FromRequest;

async fn json_response<T: serde::Serialize + Send + 'static>(
    studio: &SlideTemplates,
    status: StatusCode,
    make: impl FnOnce() -> SlideResult<T> + Send + 'static,
) -> SlideResult<Response> {
    let bytes = studio.encode_json(make).await?;
    Ok((status, [(header::CONTENT_TYPE, "application/json")], bytes).into_response())
}
