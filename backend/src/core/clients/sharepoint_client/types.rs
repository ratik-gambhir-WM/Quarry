use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type CacheFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Pluggable cache backend interface.
pub trait CacheAdapter: Send + Sync {
    fn get<'a>(&'a self, key: &'a str) -> CacheFuture<'a, Option<Value>>;
    fn set<'a>(&'a self, key: &'a str, value: Value, ttl: Option<Duration>) -> CacheFuture<'a, ()>;
    fn delete<'a>(&'a self, key: &'a str) -> CacheFuture<'a, ()>;
    fn clear(&self) -> CacheFuture<'_, ()>;
}

/// Configuration for routing user-context Graph calls through a proxy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserGraphProxyConfig {
    pub base_url: String,
    pub user_id: Option<String>,
    pub cache_key: Option<String>,
}

impl UserGraphProxyConfig {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            user_id: None,
            cache_key: None,
        }
    }
}

/// Configuration for [`crate::core::clients::sharepoint_client::SharePointClient`].
///
/// Credentials may be omitted when all used operations are routed through a
/// user Graph proxy. Direct Graph operations acquire a client-credentials token
/// lazily and return an error if credentials are missing.
#[derive(Clone, Default)]
pub struct SharePointClientConfig {
    pub tenant_id: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub cache: Option<Arc<dyn CacheAdapter>>,
    pub default_cache_ttl: Option<Duration>,
    pub user_graph_proxy: Option<UserGraphProxyConfig>,
}

/// Identifies a Microsoft Teams team and optional channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeamIdentifier {
    pub teams_id: String,
    pub channel_id: Option<String>,
}

impl TeamIdentifier {
    pub fn new(teams_id: impl Into<String>) -> Self {
        Self {
            teams_id: teams_id.into(),
            channel_id: None,
        }
    }
}

/// A Microsoft Teams team.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Team {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// A team with its resolved channel IDs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TeamChannels {
    #[serde(rename = "teamId")]
    pub team_id: String,
    pub channels: Vec<String>,
}

/// A generic SharePoint drive item returned by file-listing operations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DriveItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "webUrl")]
    pub web_url: String,
    pub size: u64,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "lastModifiedDateTime")]
    pub last_modified_date_time: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
}

/// Options for filtering drive items during recursive listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DriveItemFilterOptions {
    pub excluded_folders: Option<Vec<String>>,
    pub excluded_files: Option<Vec<String>>,
    pub excluded_extensions: Option<Vec<String>>,
}

/// Result of downloading a file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadResult {
    pub buffer: Vec<u8>,
    pub size: usize,
}

pub type IdentityExtractor<T> = Arc<dyn Fn(&T) -> String + Send + Sync>;
pub type UpdatePredicate<TNew, TExisting> = Arc<dyn Fn(&TNew, &TExisting) -> bool + Send + Sync>;

/// Options for diffing files using caller-supplied identity and update logic.
pub struct DiffOptions<TNew, TExisting> {
    pub get_new_id: IdentityExtractor<TNew>,
    pub get_existing_id: IdentityExtractor<TExisting>,
    pub should_update: Option<UpdatePredicate<TNew, TExisting>>,
}

impl<TNew, TExisting> Clone for DiffOptions<TNew, TExisting> {
    fn clone(&self) -> Self {
        Self {
            get_new_id: Arc::clone(&self.get_new_id),
            get_existing_id: Arc::clone(&self.get_existing_id),
            should_update: self.should_update.as_ref().map(Arc::clone),
        }
    }
}

/// Result of diffing two file sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileDiff<TNew, TExisting> {
    pub added: Vec<TNew>,
    pub modified: Vec<(TNew, TExisting)>,
    pub removed: Vec<TExisting>,
}

/// Combined filters and diff callbacks for a high-level file sync operation.
pub struct FileSyncOptions<TExisting> {
    pub filters: DriveItemFilterOptions,
    pub diff: DiffOptions<DriveItem, TExisting>,
}

/// Result of a file sync operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSyncResult<TExisting> {
    pub diff: FileDiff<DriveItem, TExisting>,
    pub drive_id: String,
}

/// Entity types supported by the Microsoft Graph Search API.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SharePointSearchEntityType {
    #[serde(rename = "listItem")]
    ListItem,
    #[serde(rename = "site")]
    Site,
    #[serde(rename = "list")]
    List,
    #[serde(rename = "drive")]
    Drive,
    #[serde(rename = "driveItem")]
    DriveItem,
    #[serde(rename = "externalItem")]
    ExternalItem,
}

/// Options for a SharePoint search query.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SharePointSearchOptions {
    pub query: String,
    #[serde(rename = "entityTypes")]
    pub entity_types: Vec<SharePointSearchEntityType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
}

/// Retry configuration for Graph HTTP requests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetryOptions {
    pub max_retries: Option<usize>,
    pub retry_delay: Option<Duration>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphTeamsResponse {
    pub value: Vec<Team>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphChannelFilterResponse {
    pub value: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphDriveFolderResponse {
    #[serde(rename = "parentReference")]
    pub parent_reference: GraphParentReference,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphParentReference {
    #[serde(rename = "driveId")]
    pub drive_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphDriveChildItem {
    pub id: String,
    pub name: String,
    #[serde(rename = "webUrl")]
    pub web_url: String,
    pub size: u64,
    #[serde(rename = "lastModifiedDateTime")]
    pub last_modified_date_time: String,
    pub file: Option<GraphFileFacet>,
    pub folder: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphFileFacet {
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphDriveChildrenResponse {
    #[serde(default)]
    pub value: Vec<GraphDriveChildItem>,
    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphDownloadMetadataResponse {
    #[serde(rename = "@microsoft.graph.downloadUrl")]
    pub download_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphBatchResponse {
    pub responses: Vec<GraphBatchResponseItem>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphBatchResponseItem {
    pub status: u16,
    pub id: String,
    pub body: Option<GraphBatchResponseBody>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphBatchResponseBody {
    pub value: Vec<GraphChannel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GraphChannel {
    pub id: String,
    #[serde(rename = "webUrl")]
    pub web_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: Option<String>,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct GraphErrorResponse {
    pub error: Option<GraphError>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct GraphError {
    pub message: Option<String>,
}
