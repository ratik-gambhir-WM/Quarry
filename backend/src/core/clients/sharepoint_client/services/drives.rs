use std::{collections::VecDeque, pin::Pin, time::Duration};

use futures_util::{stream, Stream};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;

use super::super::{
    error::SharePointClientError,
    types::{
        CacheAdapter, DriveItem, GraphDriveChildrenResponse, GraphDriveFolderResponse,
        GraphErrorResponse, TeamIdentifier,
    },
    utils::{
        build_drive_children_url, cache_get, cache_set, encode_uri_component, fetch_with_retry,
        normalize_file_extension, HttpRequestOptions,
    },
};

const GRAPH_BASE: &str = "https://graph.microsoft.com/v1.0";

pub type DriveItemStream =
    Pin<Box<dyn Stream<Item = Result<DriveItem, SharePointClientError>> + Send>>;

/// Resolves the drive ID for a Teams team or channel file store.
pub async fn get_drive_id(
    http_client: &reqwest::Client,
    graph_token: &str,
    team: &TeamIdentifier,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<String, SharePointClientError> {
    let cache_key = format!(
        "drive:{}:{}",
        team.teams_id,
        team.channel_id.as_deref().unwrap_or("root")
    );
    if let Some(drive_id) = cache_get(cache, &cache_key).await {
        return Ok(drive_id);
    }

    let url = match team.channel_id.as_deref() {
        Some(channel_id) => format!(
            "{GRAPH_BASE}/teams/{}/channels/{channel_id}/filesFolder",
            team.teams_id
        ),
        None => format!("{GRAPH_BASE}/teams/{}/filesFolder", team.teams_id),
    };
    let request = authorized_get(graph_token)?;
    let response = fetch_with_retry(http_client, &url, &request, None).await?;
    let data: GraphDriveFolderResponse = response.json().await?;
    let drive_id = data.parent_reference.drive_id;
    cache_set(cache, &cache_key, &drive_id, cache_ttl).await;
    Ok(drive_id)
}

/// Returns a lazy stream of non-folder drive items across all result pages.
pub fn get_drive_item_children(
    http_client: reqwest::Client,
    graph_token: String,
    drive_id: String,
    folder_path: String,
    page_size: Option<usize>,
) -> DriveItemStream {
    struct State {
        http_client: reqwest::Client,
        graph_token: String,
        next_link: Option<String>,
        pending: VecDeque<DriveItem>,
    }

    let state = State {
        http_client,
        graph_token,
        next_link: Some(build_drive_children_url(
            &drive_id,
            &folder_path,
            page_size.unwrap_or(500),
        )),
        pending: VecDeque::new(),
    };

    Box::pin(stream::try_unfold(state, |mut state| async move {
        loop {
            if let Some(item) = state.pending.pop_front() {
                return Ok(Some((item, state)));
            }

            let Some(next_link) = state.next_link.take() else {
                return Ok(None);
            };
            let request = authorized_get(&state.graph_token)?;
            let response = fetch_with_retry(&state.http_client, &next_link, &request, None).await?;
            let data: GraphDriveChildrenResponse = response.json().await?;
            state.next_link = data.next_link;
            state.pending.extend(
                data.value
                    .into_iter()
                    .filter(|item| item.folder.is_none())
                    .map(drive_item_from_graph),
            );
        }
    }))
}

/// Checks whether a specific folder path exists in a drive.
pub async fn check_folder_exists(
    http_client: &reqwest::Client,
    graph_token: &str,
    drive_id: &str,
    folder_path: &str,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<bool, SharePointClientError> {
    let cache_key = format!("folder:{drive_id}:{folder_path}");
    if let Some(exists) = cache_get(cache, &cache_key).await {
        return Ok(exists);
    }

    let url = format!(
        "{GRAPH_BASE}/drives/{drive_id}/root:{}",
        encode_uri_component(folder_path)
    );
    let response = http_client
        .get(url)
        .header(AUTHORIZATION, graph_token)
        .send()
        .await?;

    if response.status().is_success() {
        cache_set(cache, &cache_key, &true, cache_ttl).await;
        return Ok(true);
    }
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        cache_set(cache, &cache_key, &false, cache_ttl).await;
        return Ok(false);
    }

    let status = response.status();
    let status_text = status.canonical_reason().unwrap_or_default().to_owned();
    let error_data: GraphErrorResponse = response
        .json()
        .await
        .unwrap_or(GraphErrorResponse { error: None });
    let message = error_data
        .error
        .as_ref()
        .and_then(|error| error.message.as_deref())
        .unwrap_or(&status_text);
    let details = serde_json::to_value(&error_data).unwrap_or(Value::Null);
    Err(SharePointClientError::with_details(
        format!("Error checking folder: {message}"),
        status.as_u16(),
        details,
    ))
}

fn authorized_get(graph_token: &str) -> Result<HttpRequestOptions, SharePointClientError> {
    let mut headers = HeaderMap::new();
    let authorization = HeaderValue::from_str(graph_token).map_err(|error| {
        SharePointClientError::with_status(
            format!("Invalid Graph Authorization header: {error}"),
            400,
        )
    })?;
    headers.insert(AUTHORIZATION, authorization);
    Ok(HttpRequestOptions {
        headers,
        ..HttpRequestOptions::default()
    })
}

pub(crate) fn drive_item_from_graph(item: super::super::types::GraphDriveChildItem) -> DriveItem {
    DriveItem {
        id: item.id,
        name: normalize_file_extension(&item.name),
        web_url: item.web_url,
        size: item.size,
        mime_type: item.file.map(|file| file.mime_type).unwrap_or_default(),
        last_modified_date_time: item.last_modified_date_time,
        relative_path: String::new(),
    }
}
