use std::time::Duration;

use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    Method,
};
use serde_json::{json, Value};

use super::super::{
    error::SharePointClientError,
    types::{CacheAdapter, SharePointSearchEntityType, SharePointSearchOptions},
    utils::{cache_get, cache_set, fetch_with_retry, HttpRequestOptions},
};

const SEARCH_URL: &str = "https://graph.microsoft.com/v1.0/search/query";

/// Executes a SharePoint query through the Microsoft Graph Search API.
pub async fn search_sharepoint(
    http_client: &reqwest::Client,
    graph_token: &str,
    options: &SharePointSearchOptions,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<Value, SharePointClientError> {
    let serialized_options = serde_json::to_string(options)?;
    let cache_key = format!("search:{serialized_options}");
    if let Some(cached) = cache_get(cache, &cache_key).await {
        return Ok(cached);
    }

    let body = serde_json::to_vec(&json!({
        "requests": [{
            "entityTypes": options.entity_types,
            "query": { "queryString": options.query },
            "from": options.from.unwrap_or(0),
            "size": options.size.unwrap_or(5),
        }]
    }))?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(graph_token).map_err(|error| {
            SharePointClientError::with_status(
                format!("Invalid Graph Authorization header: {error}"),
                400,
            )
        })?,
    );
    let request = HttpRequestOptions {
        method: Method::POST,
        headers,
        body: Some(body),
    };
    let response = fetch_with_retry(http_client, SEARCH_URL, &request, None).await?;
    let result: Value = response.json().await?;
    cache_set(cache, &cache_key, &result, cache_ttl).await;
    Ok(result)
}

pub async fn search_files(
    http_client: &reqwest::Client,
    graph_token: &str,
    query: &str,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<Value, SharePointClientError> {
    search_sharepoint(
        http_client,
        graph_token,
        &SharePointSearchOptions {
            query: query.to_owned(),
            entity_types: vec![SharePointSearchEntityType::ListItem],
            from: None,
            size: None,
        },
        cache,
        cache_ttl,
    )
    .await
}

pub async fn search_sites(
    http_client: &reqwest::Client,
    graph_token: &str,
    query: &str,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<Value, SharePointClientError> {
    search_sharepoint(
        http_client,
        graph_token,
        &SharePointSearchOptions {
            query: query.to_owned(),
            entity_types: vec![SharePointSearchEntityType::Site],
            from: None,
            size: None,
        },
        cache,
        cache_ttl,
    )
    .await
}

pub async fn search_folders(
    http_client: &reqwest::Client,
    graph_token: &str,
    query: &str,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<Value, SharePointClientError> {
    search_sharepoint(
        http_client,
        graph_token,
        &SharePointSearchOptions {
            query: query.to_owned(),
            entity_types: vec![SharePointSearchEntityType::List],
            from: None,
            size: None,
        },
        cache,
        cache_ttl,
    )
    .await
}
