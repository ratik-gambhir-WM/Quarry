use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use reqwest::header::AUTHORIZATION;

use super::{
    super::{
        error::SharePointClientError,
        types::{
            CacheAdapter, DiffOptions, DownloadResult, DriveItem, DriveItemFilterOptions, FileDiff,
            FileSyncResult, GraphDownloadMetadataResponse, GraphDriveChildItem,
            GraphDriveChildrenResponse, TeamIdentifier,
        },
        utils::{
            build_drive_children_url, fetch_with_retry, is_path_excluded, normalize_file_extension,
            normalize_path, parse_sharepoint_folder_path, HttpRequestOptions,
        },
    },
    drives::{check_folder_exists, get_drive_id},
};

const DEFAULT_EXCLUDED_EXTENSIONS: [&str; 2] = [".mp4", ".zip"];

/// Recursively lists all files under a drive folder path.
pub async fn list_files(
    http_client: &reqwest::Client,
    graph_token: &str,
    drive_id: &str,
    folder_path: &str,
    options: Option<&DriveItemFilterOptions>,
) -> Result<Vec<DriveItem>, SharePointClientError> {
    list_files_recursive(
        http_client,
        graph_token,
        drive_id,
        folder_path,
        options,
        folder_path,
    )
    .await
}

fn list_files_recursive<'a>(
    http_client: &'a reqwest::Client,
    graph_token: &'a str,
    drive_id: &'a str,
    folder_path: &'a str,
    options: Option<&'a DriveItemFilterOptions>,
    root_path: &'a str,
) -> Pin<Box<dyn Future<Output = Result<Vec<DriveItem>, SharePointClientError>> + Send + 'a>> {
    Box::pin(async move {
        let default_extensions = DEFAULT_EXCLUDED_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect::<Vec<_>>();
        let excluded_extensions = options
            .and_then(|options| options.excluded_extensions.as_deref())
            .unwrap_or(&default_extensions);
        let mut results = Vec::new();
        let mut next_link = Some(build_drive_children_url(drive_id, folder_path, 500));

        while let Some(url) = next_link {
            let request = authorized_get(graph_token)?;
            let response = fetch_with_retry(http_client, &url, &request, None).await?;
            let data: GraphDriveChildrenResponse = response.json().await?;

            for item in data.value {
                let full_path = format!("{folder_path}/{}", item.name);
                let relative_path = relative_path_from_root(&full_path, root_path);
                let normalized_path = normalize_path(&relative_path);

                if item.folder.is_some() {
                    if is_path_excluded(
                        &normalized_path,
                        options.and_then(|options| options.excluded_folders.as_deref()),
                    ) {
                        continue;
                    }
                    let mut sub_files = list_files_recursive(
                        http_client,
                        graph_token,
                        drive_id,
                        &full_path,
                        options,
                        root_path,
                    )
                    .await?;
                    results.append(&mut sub_files);
                    continue;
                }

                if is_path_excluded(
                    &normalized_path,
                    options.and_then(|options| options.excluded_files.as_deref()),
                ) {
                    continue;
                }

                let file_name = normalize_file_extension(&item.name);
                if excluded_extensions
                    .iter()
                    .any(|extension| file_name.to_lowercase().ends_with(extension))
                {
                    continue;
                }

                results.push(drive_item_with_relative_path(
                    item,
                    file_name,
                    parent_path(&relative_path),
                ));
            }

            next_link = data.next_link;
        }

        Ok(results)
    })
}

/// Pure file diff using caller-supplied identity and update logic.
pub fn diff_files<TNew, TExisting>(
    new_files: &[TNew],
    existing_files: &[TExisting],
    options: &DiffOptions<TNew, TExisting>,
) -> FileDiff<TNew, TExisting>
where
    TNew: Clone,
    TExisting: Clone,
{
    let existing_map = existing_files
        .iter()
        .map(|file| ((options.get_existing_id)(file), file))
        .collect::<HashMap<_, _>>();
    let new_map = new_files
        .iter()
        .map(|file| ((options.get_new_id)(file), file))
        .collect::<HashMap<_, _>>();

    let added = new_files
        .iter()
        .filter(|file| !existing_map.contains_key(&(options.get_new_id)(file)))
        .cloned()
        .collect();
    let removed = existing_files
        .iter()
        .filter(|file| !new_map.contains_key(&(options.get_existing_id)(file)))
        .cloned()
        .collect();
    let mut modified = Vec::new();

    if let Some(should_update) = &options.should_update {
        for new_file in new_files {
            if let Some(existing_file) = existing_map.get(&(options.get_new_id)(new_file)) {
                if should_update(new_file, existing_file) {
                    modified.push((new_file.clone(), (*existing_file).clone()));
                }
            }
        }
    }

    FileDiff {
        added,
        modified,
        removed,
    }
}

/// Downloads a file's binary content by drive-item ID. The response is not cached.
pub async fn download_file(
    http_client: &reqwest::Client,
    graph_token: &str,
    drive_id: &str,
    item_id: &str,
) -> Result<DownloadResult, SharePointClientError> {
    let metadata_url = format!(
        "https://graph.microsoft.com/v1.0/drives/{drive_id}/items/{item_id}?select=id,@microsoft.graph.downloadUrl"
    );
    let request = authorized_get(graph_token)?;
    let metadata_response = fetch_with_retry(http_client, &metadata_url, &request, None).await?;
    let metadata: GraphDownloadMetadataResponse = metadata_response.json().await?;
    let download_url = metadata.download_url.ok_or_else(|| {
        SharePointClientError::new("Missing download URL from Graph API response")
    })?;

    let file_response = fetch_with_retry(http_client, &download_url, &request, None).await?;
    let bytes = file_response.bytes().await?.to_vec();
    Ok(DownloadResult {
        size: bytes.len(),
        buffer: bytes,
    })
}

/// Resolves the drive, lists files, and diffs them against an existing set.
#[allow(clippy::too_many_arguments)]
pub async fn get_files_for_sync<TExisting>(
    http_client: &reqwest::Client,
    graph_token: &str,
    team: &TeamIdentifier,
    sharepoint_folder_url: &str,
    existing_files: &[TExisting],
    filter_options: Option<&DriveItemFilterOptions>,
    diff_options: &DiffOptions<DriveItem, TExisting>,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<FileSyncResult<TExisting>, SharePointClientError>
where
    TExisting: Clone,
{
    let drive_id = get_drive_id(http_client, graph_token, team, cache, cache_ttl).await?;
    let relative_path = parse_sharepoint_folder_path(sharepoint_folder_url)?;
    let files = list_files(
        http_client,
        graph_token,
        &drive_id,
        &relative_path,
        filter_options,
    )
    .await?;
    let diff = diff_files(&files, existing_files, diff_options);
    Ok(FileSyncResult { diff, drive_id })
}

/// Checks whether a SharePoint folder URL resolves to an existing folder.
pub async fn check_sharepoint_folder_exists(
    http_client: &reqwest::Client,
    graph_token: &str,
    team: &TeamIdentifier,
    sharepoint_folder_url: &str,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
) -> Result<bool, SharePointClientError> {
    let drive_id = get_drive_id(http_client, graph_token, team, cache, cache_ttl).await?;
    let folder_path = parse_sharepoint_folder_path(sharepoint_folder_url)?;
    check_folder_exists(
        http_client,
        graph_token,
        &drive_id,
        &folder_path,
        cache,
        cache_ttl,
    )
    .await
}

fn authorized_get(graph_token: &str) -> Result<HttpRequestOptions, SharePointClientError> {
    let mut headers = reqwest::header::HeaderMap::new();
    let authorization = reqwest::header::HeaderValue::from_str(graph_token).map_err(|error| {
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

fn relative_path_from_root(full_path: &str, root_path: &str) -> String {
    if full_path.len() >= root_path.len()
        && full_path[..root_path.len()].eq_ignore_ascii_case(root_path)
    {
        return full_path[root_path.len()..]
            .strip_prefix('/')
            .unwrap_or(&full_path[root_path.len()..])
            .to_owned();
    }
    full_path.to_owned()
}

fn parent_path(relative_path: &str) -> String {
    relative_path
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_owned())
        .unwrap_or_default()
}

fn drive_item_with_relative_path(
    item: GraphDriveChildItem,
    name: String,
    relative_path: String,
) -> DriveItem {
    DriveItem {
        id: item.id,
        name,
        web_url: item.web_url,
        size: item.size,
        mime_type: item.file.map(|file| file.mime_type).unwrap_or_default(),
        last_modified_date_time: item.last_modified_date_time,
        relative_path,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/core/clients/sharepoint_client/services/files_tests.rs"]
mod tests;
