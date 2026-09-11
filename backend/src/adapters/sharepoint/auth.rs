use std::{sync::Arc, time::Duration};

use serde_json::Value;

use super::{
    error::SharePointClientError,
    types::{CacheAdapter, SharePointClientConfig, TokenResponse},
    utils::{cache_get, form_url_encode_component, hash_token},
};

const TOKEN_CACHE_KEY_PREFIX: &str = "sharepoint-client:oauth-token";
const EXPIRY_SAFETY_MARGIN: Duration = Duration::from_secs(60);

/// Internal token manager for client-credentials Graph tokens.
pub(crate) struct TokenManager {
    tenant_id: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    cache: Arc<dyn CacheAdapter>,
    http_client: reqwest::Client,
}

impl TokenManager {
    pub(crate) fn new(
        config: &SharePointClientConfig,
        cache: Arc<dyn CacheAdapter>,
        http_client: reqwest::Client,
    ) -> Self {
        Self {
            tenant_id: config.tenant_id.clone(),
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            cache,
            http_client,
        }
    }

    /// Returns a valid Graph API token, refreshing if needed.
    pub(crate) async fn get_token(&self) -> Result<String, SharePointClientError> {
        let credentials = self.credentials()?;
        let cache_key = self.cache_key(credentials);
        if let Some(token) = cache_get::<String>(self.cache.as_ref(), &cache_key).await {
            return Ok(token);
        }

        let form_body = [
            ("client_id", credentials.client_id),
            ("scope", "https://graph.microsoft.com/.default"),
            ("client_secret", credentials.client_secret),
            ("grant_type", "client_credentials"),
        ]
        .into_iter()
        .map(|(key, value)| format!("{key}={}", form_url_encode_component(value)))
        .collect::<Vec<_>>()
        .join("&");
        let response = self
            .http_client
            .post(format!(
                "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
                credentials.tenant_id
            ))
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(SharePointClientError::with_details(
                format!("Failed to acquire Graph token: {}", status.as_u16()),
                status.as_u16(),
                body,
            ));
        }

        let data: TokenResponse = response.json().await?;
        let access_token = data.access_token.ok_or_else(|| {
            SharePointClientError::with_details(
                "Token response missing access_token",
                500,
                Value::Null,
            )
        })?;

        let ttl = Duration::from_secs(data.expires_in).saturating_sub(EXPIRY_SAFETY_MARGIN);
        self.cache
            .set(&cache_key, Value::String(access_token.clone()), Some(ttl))
            .await;
        Ok(access_token)
    }

    fn cache_key(&self, credentials: Credentials<'_>) -> String {
        let credential_key = format!(
            "{}:{}:{}",
            credentials.tenant_id, credentials.client_id, credentials.client_secret
        );
        format!("{TOKEN_CACHE_KEY_PREFIX}:{}", hash_token(&credential_key))
    }

    fn credentials(&self) -> Result<Credentials<'_>, SharePointClientError> {
        match (
            self.tenant_id.as_deref(),
            self.client_id.as_deref(),
            self.client_secret.as_deref(),
        ) {
            (Some(tenant_id), Some(client_id), Some(client_secret))
                if !tenant_id.is_empty() && !client_id.is_empty() && !client_secret.is_empty() =>
            {
                Ok(Credentials {
                    tenant_id,
                    client_id,
                    client_secret,
                })
            }
            _ => Err(SharePointClientError::with_status(
                "tenantId, clientId, and clientSecret are required for direct Microsoft Graph token acquisition.",
                400,
            )),
        }
    }
}

#[derive(Clone, Copy)]
struct Credentials<'a> {
    tenant_id: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}
