use reqwest::header::{HeaderValue, AUTHORIZATION};

use super::{
    error::SharePointClientError,
    types::UserGraphProxyConfig,
    utils::{encode_uri_component, HttpRequestOptions},
};

pub const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";

#[derive(Clone, Copy, Debug, Default)]
pub struct GraphRequestContext<'a> {
    pub token: Option<&'a str>,
    pub user_graph_proxy: Option<&'a UserGraphProxyConfig>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GraphRequestOptions {
    pub requires_user_context: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphRequest {
    pub url: String,
    pub request: HttpRequestOptions,
}

/// True for Graph paths that are explicitly scoped to a user.
pub fn is_user_scoped_graph_path(path_or_url: &str) -> bool {
    let path = normalize_graph_path(path_or_url);
    let (pathname, _) = split_path_and_suffix(&path);
    if pathname == "/me" || pathname.starts_with("/me/") {
        return true;
    }

    pathname
        .strip_prefix("/users/")
        .and_then(|tail| tail.split('/').next())
        .is_some_and(|user_id| !user_id.is_empty())
}

/// Builds a Graph URL, using the user proxy only for user-context requests.
pub fn build_graph_url(
    path_or_url: &str,
    context: GraphRequestContext<'_>,
    options: GraphRequestOptions,
) -> String {
    let path = normalize_graph_path(path_or_url);
    if should_use_user_graph_proxy(&path, context, options) {
        return join_url(
            context
                .user_graph_proxy
                .map(|proxy| proxy.base_url.as_str())
                .unwrap_or_default(),
            &build_user_graph_proxy_path(&path, context.user_graph_proxy),
        );
    }
    join_url(GRAPH_BASE_URL, &path)
}

/// Builds an HTTP request and strips authorization when a proxy is used.
pub(crate) fn build_graph_request(
    path_or_url: &str,
    context: GraphRequestContext<'_>,
    mut init: HttpRequestOptions,
    options: GraphRequestOptions,
) -> Result<GraphRequest, SharePointClientError> {
    let path = normalize_graph_path(path_or_url);
    let use_proxy = should_use_user_graph_proxy(&path, context, options);

    if !use_proxy && context.token.is_none() {
        return Err(SharePointClientError::new(
            "A Graph token is required for direct Microsoft Graph requests.",
        ));
    }

    if use_proxy {
        init.headers.remove(AUTHORIZATION);
    } else {
        let token = HeaderValue::from_str(context.token.unwrap_or_default()).map_err(|error| {
            SharePointClientError::with_status(
                format!("Invalid Graph Authorization header: {error}"),
                400,
            )
        })?;
        init.headers.insert(AUTHORIZATION, token);
    }

    Ok(GraphRequest {
        url: build_graph_url(&path, context, options),
        request: init,
    })
}

pub fn should_use_user_graph_proxy(
    path_or_url: &str,
    context: GraphRequestContext<'_>,
    options: GraphRequestOptions,
) -> bool {
    context.user_graph_proxy.is_some()
        && (options.requires_user_context || is_user_scoped_graph_path(path_or_url))
}

fn build_user_graph_proxy_path(path: &str, proxy: Option<&UserGraphProxyConfig>) -> String {
    let (pathname, suffix) = split_path_and_suffix(path);
    let default_user_id = encode_uri_component(
        proxy
            .and_then(|proxy| proxy.user_id.as_deref())
            .unwrap_or("me"),
    );

    if pathname == "/me" || pathname.starts_with("/me/") {
        let tail = &pathname["/me".len()..];
        return format!("/users/{default_user_id}/graph{tail}{suffix}");
    }

    if let Some(users_tail) = pathname.strip_prefix("/users/") {
        let (user_id, tail) = users_tail
            .split_once('/')
            .map(|(user_id, tail)| (user_id, format!("/{tail}")))
            .unwrap_or((users_tail, String::new()));
        if !user_id.is_empty() {
            return format!("/users/{user_id}/graph{tail}{suffix}");
        }
    }

    format!("/users/{default_user_id}/graph{pathname}{suffix}")
}

fn normalize_graph_path(path_or_url: &str) -> String {
    if let Some(path) = path_or_url.strip_prefix(GRAPH_BASE_URL) {
        return if path.is_empty() {
            "/".to_owned()
        } else {
            path.to_owned()
        };
    }
    if path_or_url.starts_with('/') {
        path_or_url.to_owned()
    } else {
        format!("/{path_or_url}")
    }
}

fn split_path_and_suffix(path: &str) -> (&str, &str) {
    path.find(['?', '#'])
        .map(|index| path.split_at(index))
        .unwrap_or((path, ""))
}

fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[cfg(test)]
#[path = "../../../../tests/core/clients/sharepoint_client/graph_tests.rs"]
mod tests;
