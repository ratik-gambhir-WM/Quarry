use std::{sync::Arc, time::Duration};

use serde_json::Value;

use super::{
    auth::TokenManager,
    cache::InMemoryCache,
    error::SharePointClientError,
    services::{drives, files, search, teams},
    types::{
        CacheAdapter, DiffOptions, DownloadResult, DriveItem, DriveItemFilterOptions, FileDiff,
        FileSyncOptions, FileSyncResult, SharePointClientConfig, SharePointSearchOptions, Team,
        TeamChannels, TeamIdentifier, UserGraphProxyConfig,
    },
};

const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

/// Entry point for all SharePoint / Microsoft Graph operations.
///
/// Handles credentials, token refresh, and response caching. Every Graph
/// operation accepts an explicit token override for user-delegated calls.
pub struct SharePointClient {
    token_manager: TokenManager,
    cache: Arc<dyn CacheAdapter>,
    cache_ttl: Duration,
    user_graph_proxy: Option<UserGraphProxyConfig>,
    http_client: reqwest::Client,
}

impl SharePointClient {
    pub fn new(config: SharePointClientConfig) -> Self {
        Self::with_http_client(config, reqwest::Client::new())
    }

    /// Constructs the client with a caller-provided HTTP client.
    ///
    /// This is useful when a future integration needs shared connection pools,
    /// timeouts, or test middleware.
    pub fn with_http_client(config: SharePointClientConfig, http_client: reqwest::Client) -> Self {
        let cache = config
            .cache
            .clone()
            .unwrap_or_else(|| Arc::new(InMemoryCache::default()));
        let token_manager = TokenManager::new(&config, Arc::clone(&cache), http_client.clone());
        Self {
            token_manager,
            cache,
            cache_ttl: config.default_cache_ttl.unwrap_or(DEFAULT_CACHE_TTL),
            user_graph_proxy: config.user_graph_proxy,
            http_client,
        }
    }

    /// Acquires or returns a cached Graph API token.
    pub async fn acquire_token(&self) -> Result<String, SharePointClientError> {
        self.token_manager.get_token().await
    }

    /// Clears cached tokens, drive IDs, and Graph responses.
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }

    pub async fn get_joined_teams(
        &self,
        graph_token: Option<&str>,
    ) -> Result<Vec<Team>, SharePointClientError> {
        let token = self.resolve_user_graph_token(graph_token).await?;
        teams::get_joined_teams(
            &self.http_client,
            token.as_deref(),
            self.cache.as_ref(),
            self.cache_ttl,
            self.user_graph_proxy.as_ref(),
        )
        .await
    }

    pub async fn is_channel_member(
        &self,
        team: &TeamIdentifier,
        graph_token: Option<&str>,
    ) -> Result<bool, SharePointClientError> {
        let token = self.resolve_user_graph_token(graph_token).await?;
        teams::is_channel_member(
            &self.http_client,
            token.as_deref(),
            team,
            self.user_graph_proxy.as_ref(),
        )
        .await
    }

    pub async fn is_team_member(
        &self,
        team: &TeamIdentifier,
        graph_token: Option<&str>,
    ) -> Result<bool, SharePointClientError> {
        let token = self.resolve_user_graph_token(graph_token).await?;
        teams::is_team_member(
            &self.http_client,
            token.as_deref(),
            team,
            self.cache.as_ref(),
            self.cache_ttl,
            self.user_graph_proxy.as_ref(),
        )
        .await
    }

    pub async fn get_teams_with_channels(
        &self,
        team_ids: &[String],
        batch_size: Option<usize>,
        graph_token: Option<&str>,
    ) -> Result<Vec<TeamChannels>, SharePointClientError> {
        let token = self.resolve_user_graph_token(graph_token).await?;
        teams::get_teams_with_channels(
            &self.http_client,
            token.as_deref(),
            team_ids,
            self.cache.as_ref(),
            self.cache_ttl,
            batch_size,
            self.user_graph_proxy.as_ref(),
        )
        .await
    }

    pub fn is_team_and_channel_member(
        &self,
        teams_with_channels: &[TeamChannels],
        teams_id: &str,
        channel_id: &str,
    ) -> bool {
        teams::is_team_and_channel_member(teams_with_channels, teams_id, channel_id)
    }

    pub async fn get_drive_id(
        &self,
        team: &TeamIdentifier,
        graph_token: Option<&str>,
    ) -> Result<String, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        drives::get_drive_id(
            &self.http_client,
            &token,
            team,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    pub async fn get_drive_item_children(
        &self,
        drive_id: &str,
        folder_path: &str,
        page_size: Option<usize>,
        graph_token: Option<&str>,
    ) -> Result<drives::DriveItemStream, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        Ok(drives::get_drive_item_children(
            self.http_client.clone(),
            token,
            drive_id.to_owned(),
            folder_path.to_owned(),
            page_size,
        ))
    }

    pub async fn check_folder_exists(
        &self,
        drive_id: &str,
        folder_path: &str,
        graph_token: Option<&str>,
    ) -> Result<bool, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        drives::check_folder_exists(
            &self.http_client,
            &token,
            drive_id,
            folder_path,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    pub async fn list_files(
        &self,
        drive_id: &str,
        folder_path: &str,
        options: Option<&DriveItemFilterOptions>,
        graph_token: Option<&str>,
    ) -> Result<Vec<DriveItem>, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        files::list_files(&self.http_client, &token, drive_id, folder_path, options).await
    }

    pub fn diff_files<TNew, TExisting>(
        &self,
        new_files: &[TNew],
        existing_files: &[TExisting],
        options: &DiffOptions<TNew, TExisting>,
    ) -> FileDiff<TNew, TExisting>
    where
        TNew: Clone,
        TExisting: Clone,
    {
        files::diff_files(new_files, existing_files, options)
    }

    pub async fn download_file(
        &self,
        drive_id: &str,
        item_id: &str,
        graph_token: Option<&str>,
    ) -> Result<DownloadResult, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        files::download_file(&self.http_client, &token, drive_id, item_id).await
    }

    pub async fn get_files_for_sync<TExisting>(
        &self,
        team: &TeamIdentifier,
        sharepoint_folder_url: &str,
        existing_files: &[TExisting],
        options: &FileSyncOptions<TExisting>,
        graph_token: Option<&str>,
    ) -> Result<FileSyncResult<TExisting>, SharePointClientError>
    where
        TExisting: Clone,
    {
        let token = self.resolve_token(graph_token).await?;
        files::get_files_for_sync(
            &self.http_client,
            &token,
            team,
            sharepoint_folder_url,
            existing_files,
            Some(&options.filters),
            &options.diff,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    pub async fn check_sharepoint_folder_exists(
        &self,
        team: &TeamIdentifier,
        sharepoint_folder_url: &str,
        graph_token: Option<&str>,
    ) -> Result<bool, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        files::check_sharepoint_folder_exists(
            &self.http_client,
            &token,
            team,
            sharepoint_folder_url,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    pub async fn search_sharepoint(
        &self,
        options: &SharePointSearchOptions,
        graph_token: Option<&str>,
    ) -> Result<Value, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        search::search_sharepoint(
            &self.http_client,
            &token,
            options,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    pub async fn search_files(
        &self,
        query: &str,
        graph_token: Option<&str>,
    ) -> Result<Value, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        search::search_files(
            &self.http_client,
            &token,
            query,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    pub async fn search_sites(
        &self,
        query: &str,
        graph_token: Option<&str>,
    ) -> Result<Value, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        search::search_sites(
            &self.http_client,
            &token,
            query,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    pub async fn search_folders(
        &self,
        query: &str,
        graph_token: Option<&str>,
    ) -> Result<Value, SharePointClientError> {
        let token = self.resolve_token(graph_token).await?;
        search::search_folders(
            &self.http_client,
            &token,
            query,
            self.cache.as_ref(),
            self.cache_ttl,
        )
        .await
    }

    async fn resolve_token(
        &self,
        graph_token: Option<&str>,
    ) -> Result<String, SharePointClientError> {
        match graph_token {
            Some(token) => Ok(token.to_owned()),
            None => self.token_manager.get_token().await,
        }
    }

    async fn resolve_user_graph_token(
        &self,
        graph_token: Option<&str>,
    ) -> Result<Option<String>, SharePointClientError> {
        if self.user_graph_proxy.is_some() {
            return Ok(graph_token.map(str::to_owned));
        }
        self.resolve_token(graph_token).await.map(Some)
    }
}

#[cfg(test)]
#[path = "../../../../tests/core/clients/sharepoint_client/client_tests.rs"]
mod tests;
