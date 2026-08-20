use std::time::Duration;

use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_TYPE},
    Method,
};
use serde_json::json;

use super::super::{
    error::SharePointClientError,
    graph::{build_graph_request, GraphRequestContext, GraphRequestOptions},
    types::{
        CacheAdapter, GraphBatchResponse, GraphChannelFilterResponse, GraphTeamsResponse, Team,
        TeamChannels, TeamIdentifier, UserGraphProxyConfig,
    },
    utils::{
        cache_get, cache_set, encode_uri_component, fetch_with_retry, hash_token,
        HttpRequestOptions,
    },
};

const DEFAULT_BATCH_SIZE: usize = 20;

/// Fetches all teams the authenticated user has joined.
pub async fn get_joined_teams(
    http_client: &reqwest::Client,
    graph_token: Option<&str>,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
    user_graph_proxy: Option<&UserGraphProxyConfig>,
) -> Result<Vec<Team>, SharePointClientError> {
    let cache_key = user_graph_cache_key("teams:joined", graph_token, user_graph_proxy);
    if let Some(cache_key) = &cache_key {
        if let Some(teams) = cache_get(cache, cache_key).await {
            return Ok(teams);
        }
    }

    let request = build_graph_request(
        "/me/joinedTeams",
        GraphRequestContext {
            token: graph_token,
            user_graph_proxy,
        },
        HttpRequestOptions::default(),
        GraphRequestOptions {
            requires_user_context: true,
        },
    )?;
    let response = fetch_with_retry(http_client, &request.url, &request.request, None).await?;
    let data: GraphTeamsResponse = response.json().await?;

    if let Some(cache_key) = &cache_key {
        cache_set(cache, cache_key, &data.value, cache_ttl).await;
    }
    Ok(data.value)
}

/// Checks whether the token holder is a member of a specific channel.
pub async fn is_channel_member(
    http_client: &reqwest::Client,
    graph_token: Option<&str>,
    team: &TeamIdentifier,
    user_graph_proxy: Option<&UserGraphProxyConfig>,
) -> Result<bool, SharePointClientError> {
    let Some(channel_id) = team.channel_id.as_deref() else {
        return Ok(true);
    };

    let request = build_graph_request(
        &format!(
            "/teams/{}/channels?$filter=id eq '{}'",
            team.teams_id, channel_id
        ),
        GraphRequestContext {
            token: graph_token,
            user_graph_proxy,
        },
        HttpRequestOptions::default(),
        GraphRequestOptions {
            requires_user_context: true,
        },
    )?;
    let response = fetch_with_retry(http_client, &request.url, &request.request, None).await?;
    let data: GraphChannelFilterResponse = response.json().await?;
    Ok(!data.value.is_empty())
}

/// Checks whether the token holder is a member of a team and optional channel.
pub async fn is_team_member(
    http_client: &reqwest::Client,
    graph_token: Option<&str>,
    team: &TeamIdentifier,
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
    user_graph_proxy: Option<&UserGraphProxyConfig>,
) -> Result<bool, SharePointClientError> {
    let teams =
        get_joined_teams(http_client, graph_token, cache, cache_ttl, user_graph_proxy).await?;
    if !teams.iter().any(|joined| joined.id == team.teams_id) {
        return Ok(false);
    }

    if team.channel_id.is_some() {
        return is_channel_member(http_client, graph_token, team, user_graph_proxy).await;
    }
    Ok(true)
}

/// Fetches channels for multiple teams through the Graph batch API.
pub async fn get_teams_with_channels(
    http_client: &reqwest::Client,
    graph_token: Option<&str>,
    team_ids: &[String],
    cache: &dyn CacheAdapter,
    cache_ttl: Duration,
    batch_size: Option<usize>,
    user_graph_proxy: Option<&UserGraphProxyConfig>,
) -> Result<Vec<TeamChannels>, SharePointClientError> {
    if team_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut sorted_team_ids = team_ids.to_vec();
    sorted_team_ids.sort();
    let cache_scope = user_graph_cache_key("teams:channels", graph_token, user_graph_proxy);
    let cache_key = cache_scope.map(|scope| format!("{scope}:{}", sorted_team_ids.join(",")));
    if let Some(cache_key) = &cache_key {
        if let Some(teams) = cache_get(cache, cache_key).await {
            return Ok(teams);
        }
    }

    let batch_size = batch_size.unwrap_or(DEFAULT_BATCH_SIZE);
    if batch_size == 0 {
        return Err(SharePointClientError::with_status(
            "batch_size must be greater than zero",
            400,
        ));
    }

    let mut results = Vec::new();
    for batch in team_ids.chunks(batch_size) {
        let requests = batch
            .iter()
            .enumerate()
            .map(|(index, team_id)| {
                json!({
                    "id": index.to_string(),
                    "method": "GET",
                    "url": format!("/teams/{team_id}/channels?$select=id,webUrl"),
                })
            })
            .collect::<Vec<_>>();
        let body = serde_json::to_vec(&json!({ "requests": requests }))?;
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let request = build_graph_request(
            "/$batch",
            GraphRequestContext {
                token: graph_token,
                user_graph_proxy,
            },
            HttpRequestOptions {
                method: Method::POST,
                headers,
                body: Some(body),
            },
            GraphRequestOptions {
                requires_user_context: true,
            },
        )?;
        let response = fetch_with_retry(http_client, &request.url, &request.request, None).await?;
        let data: GraphBatchResponse = response.json().await?;

        for response in data.responses {
            if response.status != 200 {
                continue;
            }
            let Some(body) = response.body else {
                continue;
            };
            let Ok(batch_index) = response.id.parse::<usize>() else {
                continue;
            };
            let Some(team_id) = batch.get(batch_index) else {
                continue;
            };
            let channels = body
                .value
                .into_iter()
                .filter(|channel| channel.web_url.is_some())
                .map(|channel| encode_uri_component(&channel.id))
                .collect();
            results.push(TeamChannels {
                team_id: team_id.clone(),
                channels,
            });
        }
    }

    if let Some(cache_key) = &cache_key {
        cache_set(cache, cache_key, &results, cache_ttl).await;
    }
    Ok(results)
}

/// Pure membership check against pre-fetched team/channel data.
pub fn is_team_and_channel_member(
    teams_with_channels: &[TeamChannels],
    teams_id: &str,
    channel_id: &str,
) -> bool {
    teams_with_channels
        .iter()
        .find(|team| team.team_id == teams_id)
        .is_some_and(|team| team.channels.iter().any(|channel| channel == channel_id))
}

fn user_graph_cache_key(
    key_prefix: &str,
    graph_token: Option<&str>,
    user_graph_proxy: Option<&UserGraphProxyConfig>,
) -> Option<String> {
    let Some(proxy) = user_graph_proxy else {
        return graph_token.map(|token| format!("{key_prefix}:{}", hash_token(token)));
    };

    let proxy_cache_key = proxy.cache_key.clone().or_else(|| {
        proxy
            .user_id
            .as_ref()
            .filter(|user_id| user_id.as_str() != "me")
            .map(|user_id| format!("user:{user_id}"))
    });
    proxy_cache_key.map(|cache_key| format!("{key_prefix}:proxy:{cache_key}"))
}

#[cfg(test)]
#[path = "../../../../../tests/core/clients/sharepoint_client/services/teams_tests.rs"]
mod tests;
