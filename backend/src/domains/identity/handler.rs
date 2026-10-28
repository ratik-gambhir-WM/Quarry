use super::{
    model::*,
    service::{timestamp, IdentityService},
};
use crate::app::http::error::{AppError, AppResult};
use axum::{
    extract::{rejection::JsonRejection, ConnectInfo, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    Extension, Json,
};
use std::{net::SocketAddr, sync::Arc};

fn username(value: &mut String) -> AppResult<()> {
    *value = value.trim().to_ascii_lowercase();
    if !(3..=32).contains(&value.len())
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(AppError::bad_request(
            "username must be 3–32 letters, digits, dots, underscores or hyphens",
        ));
    }
    Ok(())
}
fn password(value: &str) -> AppResult<()> {
    if !(15..=128).contains(&value.chars().count()) || value.len() > 512 {
        return Err(AppError::bad_request(
            "password must be 15–128 characters and at most 512 bytes",
        ));
    }
    Ok(())
}
fn text(value: &str, max: usize) -> AppResult<()> {
    if value.trim().is_empty() || value.chars().count() > max || value.chars().any(char::is_control)
    {
        return Err(AppError::bad_request("invalid profile field"));
    }
    Ok(())
}
fn no_store() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h
}
fn peer(
    identity: &IdentityService,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> AppResult<()> {
    identity
        .admit(
            format!(
                "peer:{}",
                peer.map(|p| p.0 .0.ip().to_string())
                    .unwrap_or_else(|| "local-test".into())
            ),
            20,
        )
        .map_err(Into::into)
}
pub(super) async fn register(
    State(identity): State<Arc<IdentityService>>,
    peer_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    input: Result<Json<RegisterInput>, JsonRejection>,
) -> AppResult<(StatusCode, HeaderMap, Json<SessionResponse>)> {
    peer(&identity, peer_info)?;
    let Json(mut input) =
        input.map_err(|_| AppError::bad_request("invalid registration request"))?;
    username(&mut input.username)?;
    password(&input.password)?;
    input.email = input.email.trim().to_ascii_lowercase();
    text(&input.email, 320)?;
    let (local, domain) = input
        .email
        .split_once('@')
        .ok_or_else(|| AppError::bad_request("invalid email"))?;
    if local.is_empty()
        || !domain.contains('.')
        || domain.contains('@')
        || input.email.chars().any(char::is_whitespace)
    {
        return Err(AppError::bad_request("invalid email"));
    }
    if domain != "westmonroe.com" {
        return Err(AppError::bad_request(
            "use your @westmonroe.com email address",
        ));
    }
    text(&input.first_name, 100)?;
    text(&input.last_name, 100)?;
    input.first_name = input.first_name.trim().into();
    input.last_name = input.last_name.trim().into();
    if let Some(title) = &mut input.job_title {
        text(title, 200)?;
        *title = title.trim().into();
    }
    Ok((
        StatusCode::CREATED,
        no_store(),
        Json(identity.register(input).await?),
    ))
}
pub(super) async fn login(
    State(identity): State<Arc<IdentityService>>,
    peer_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    input: Result<Json<LoginInput>, JsonRejection>,
) -> AppResult<(HeaderMap, Json<SessionResponse>)> {
    peer(&identity, peer_info)?;
    let Json(mut input) = input.map_err(|_| AppError::bad_request("invalid login request"))?;
    // Login policy failures deliberately share the invalid-credentials response.
    username(&mut input.username).map_err(|_| AppError::Unauthorized)?;
    password(&input.password).map_err(|_| AppError::Unauthorized)?;
    Ok((no_store(), Json(identity.login(input).await?)))
}
pub(super) async fn me(
    Extension(actor): Extension<AuthenticatedUser>,
) -> (HeaderMap, Json<serde_json::Value>) {
    (
        no_store(),
        Json(serde_json::json!({"user": actor.user, "expiresAt": timestamp(actor.expires_at)})),
    )
}
pub(super) async fn logout(
    State(identity): State<Arc<IdentityService>>,
    Extension(actor): Extension<AuthenticatedUser>,
) -> AppResult<(StatusCode, HeaderMap)> {
    identity.logout(&actor).await?;
    Ok((StatusCode::NO_CONTENT, no_store()))
}
