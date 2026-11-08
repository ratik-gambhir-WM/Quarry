use super::super::{error::FailureKind, SlideResult, SlideTemplateError, TemplateKind};
use axum::http::{header, HeaderMap};

pub(super) fn invalid(message: &'static str) -> SlideTemplateError {
    SlideTemplateError::new(FailureKind::Invalid, "invalid_request", message)
}

pub(super) fn validate_id(id: &str) -> SlideResult<()> {
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

pub(super) fn query(
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    allowed: &[&str],
) -> SlideResult<Vec<(String, String)>> {
    let pairs = url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .into_owned()
        .collect::<Vec<_>>();
    for (index, (key, value)) in pairs.iter().enumerate() {
        if key == "appId" {
            if value != super::super::APP_ID {
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
            .is_some_and(|h| h.to_str().ok() != Some(super::super::APP_ID))
    {
        return Err(invalid("The application scope is fixed by the server."));
    }
    Ok(pairs)
}

pub(super) fn kind(pairs: &[(String, String)]) -> SlideResult<TemplateKind> {
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

pub(super) fn headers(headers: &HeaderMap, mime: &str, limit: usize) -> SlideResult<()> {
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

pub(super) fn too_large() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::TooLarge,
        "payload_too_large",
        "The request exceeds the processing limit.",
    )
}
