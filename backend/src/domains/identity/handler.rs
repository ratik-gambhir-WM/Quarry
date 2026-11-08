use validate::{email, password, text, username};
mod validate;
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
    email(&mut input.email)?;
    if !input.email.ends_with("@westmonroe.com") {
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
    if input.username.contains('@') {
        email(&mut input.username)
    } else {
        username(&mut input.username)
    }
    .map_err(|_| AppError::Unauthorized)?;
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
