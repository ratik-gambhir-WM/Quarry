use std::time::Duration;

use reqwest::{header::HeaderMap, Method, Response};
use sha2::{Digest, Sha256};

use super::{
    error::SharePointClientError,
    types::{CacheAdapter, RetryOptions},
};

const DEFAULT_MAX_RETRIES: usize = 10;
const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(1);
const NON_RETRIABLE_STATUS_CODES: [u16; 4] = [400, 401, 403, 404];

#[derive(Clone, Debug)]
pub(crate) struct HttpRequestOptions {
    pub method: Method,
    pub headers: HeaderMap,
    pub body: Option<Vec<u8>>,
}

impl Default for HttpRequestOptions {
    fn default() -> Self {
        Self {
            method: Method::GET,
            headers: HeaderMap::new(),
            body: None,
        }
    }
}

fn should_retry_status(status: u16) -> bool {
    !NON_RETRIABLE_STATUS_CODES.contains(&status)
}

/// Performs an HTTP request with automatic retries for retryable statuses.
pub(crate) async fn fetch_with_retry(
    client: &reqwest::Client,
    url: &str,
    options: &HttpRequestOptions,
    retry_options: Option<RetryOptions>,
) -> Result<Response, SharePointClientError> {
    let max_retries = retry_options
        .and_then(|options| options.max_retries)
        .unwrap_or(DEFAULT_MAX_RETRIES);
    let retry_delay = retry_options
        .and_then(|options| options.retry_delay)
        .unwrap_or(DEFAULT_RETRY_DELAY);

    for attempt in 0..=max_retries {
        let mut request = client
            .request(options.method.clone(), url)
            .headers(options.headers.clone());
        if let Some(body) = &options.body {
            request = request.body(body.clone());
        }

        let response = request.send().await?;
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status().as_u16();
        if !should_retry_status(status) || attempt == max_retries {
            return Err(create_http_error(response, max_retries, url).await);
        }

        let delay = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
            .map(Duration::from_secs_f64)
            .unwrap_or(retry_delay);
        tokio::time::sleep(delay).await;
    }

    Err(SharePointClientError::new(format!(
        "Exceeded maximum retries for {url}"
    )))
}

async fn create_http_error(
    response: Response,
    max_retries: usize,
    url: &str,
) -> SharePointClientError {
    let status = response.status().as_u16();
    let retry_text = if should_retry_status(status) {
        format!("exceeded {max_retries} retries")
    } else {
        "not retryable".to_owned()
    };
    let body = response
        .text()
        .await
        .unwrap_or_default()
        .trim()
        .chars()
        .take(1_000)
        .collect::<String>();
    let details = if body.is_empty() {
        String::new()
    } else {
        format!(": {body}")
    };

    if body.is_empty() {
        SharePointClientError::with_status(
            format!("HTTP {status}: {retry_text} for {url}{details}"),
            status,
        )
    } else {
        SharePointClientError::with_details(
            format!("HTTP {status}: {retry_text} for {url}{details}"),
            status,
            body,
        )
    }
}

/// Parses a SharePoint folder URL into its relative path component.
pub fn parse_sharepoint_folder_path(
    sharepoint_folder_url: &str,
) -> Result<String, SharePointClientError> {
    let cleaned = sharepoint_folder_url.replacen("/:f:/r/", "/", 1);
    let without_query = cleaned.split('?').next().unwrap_or(&cleaned);
    let without_fragment = without_query.split('#').next().unwrap_or(without_query);
    let scheme_end = without_fragment
        .find("://")
        .ok_or_else(|| SharePointClientError::with_status("Invalid SharePoint folder URL", 400))?;
    let authority_and_path = &without_fragment[scheme_end + 3..];
    let encoded_path = authority_and_path
        .find('/')
        .map(|index| &authority_and_path[index..])
        .unwrap_or("/");
    let full_path = percent_decode(encoded_path)?;

    for marker in ["/Shared Documents/", "@thread.tacv2/"] {
        if let Some(index) = full_path.find(marker) {
            return Ok(format!("/{}", &full_path[index + marker.len()..]));
        }
    }

    Ok(full_path)
}

/// Trims leading/trailing slashes and lowercases a path.
pub fn normalize_path(path: &str) -> String {
    path.trim_matches('/').to_lowercase()
}

/// Checks whether a normalized path matches an exclusion entry.
pub fn is_path_excluded(normalized_path: &str, exclusions: Option<&[String]>) -> bool {
    exclusions.is_some_and(|exclusions| {
        exclusions
            .iter()
            .any(|excluded| normalize_path(excluded) == normalized_path)
    })
}

/// Lowercases the final file extension while preserving base-name casing.
pub fn normalize_file_extension(filename: &str) -> String {
    let Some(last_dot) = filename.rfind('.') else {
        return filename.to_owned();
    };
    format!(
        "{}{}",
        &filename[..last_dot],
        filename[last_dot..].to_lowercase()
    )
}

/// Builds the Graph URL for listing folder children.
pub fn build_drive_children_url(drive_id: &str, folder_path: &str, page_size: usize) -> String {
    format!(
        "https://graph.microsoft.com/v1.0/drives/{drive_id}/root:{}:/children?$top={page_size}",
        encode_uri_component(folder_path)
    )
}

/// Returns a SHA-256 cache namespace without exposing the source token.
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) async fn cache_get<T: serde::de::DeserializeOwned>(
    cache: &dyn CacheAdapter,
    key: &str,
) -> Option<T> {
    cache
        .get(key)
        .await
        .and_then(|value| serde_json::from_value(value).ok())
}

pub(crate) async fn cache_set<T: serde::Serialize>(
    cache: &dyn CacheAdapter,
    key: &str,
    value: &T,
    ttl: Duration,
) {
    if let Ok(value) = serde_json::to_value(value) {
        cache.set(key, value, Some(ttl)).await;
    }
}

pub(crate) fn encode_uri_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
            )
        {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

pub(crate) fn form_url_encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'*' | b'-' | b'.' | b'_') {
            encoded.push(char::from(byte));
        } else if byte == b' ' {
            encoded.push('+');
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn percent_decode(value: &str) -> Result<String, SharePointClientError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(SharePointClientError::with_status(
                    "Invalid percent encoding in SharePoint folder URL",
                    400,
                ));
            }
            let high = hex_value(bytes[index + 1]);
            let low = hex_value(bytes[index + 2]);
            let (Some(high), Some(low)) = (high, low) else {
                return Err(SharePointClientError::with_status(
                    "Invalid percent encoding in SharePoint folder URL",
                    400,
                ));
            };
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).map_err(|_| {
        SharePointClientError::with_status("Invalid UTF-8 in SharePoint folder URL", 400)
    })
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../../../tests/core/clients/sharepoint_client/utils_tests.rs"]
mod tests;
