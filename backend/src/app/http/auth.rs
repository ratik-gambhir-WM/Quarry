use crate::{
    app::http::error::{AppError, AppResult},
    domains::identity::{access::ResourceAccess, service::IdentityService},
};
use axum::{
    body::Body,
    extract::{FromRequestParts, Path, Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use futures_util::StreamExt;
use std::{collections::HashMap, sync::Arc, time::Duration};
#[derive(Clone)]
pub(crate) struct AuthBoundary {
    pub identity: Arc<IdentityService>,
    pub access: ResourceAccess,
}
pub(crate) async fn authenticate(
    State(boundary): State<AuthBoundary>,
    request: Request,
    next: Next,
) -> AppResult<Response> {
    let (mut parts, body) = request.into_parts();
    let mut stream_deal = None;
    if parts.headers.get_all(header::AUTHORIZATION).iter().count() != 1
        || parts.headers.get_all("x-user-id").iter().count() != 1
    {
        return Err(AppError::Unauthorized);
    }
    let token = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    let actor = boundary.identity.authenticate(token).await?;
    let supplied_id = parts
        .headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|id| *id > 0)
        .ok_or(AppError::Unauthorized)?;
    if supplied_id != actor.user.id {
        return Err(AppError::Forbidden);
    }
    if let Ok(Path(params)) =
        Path::<HashMap<String, String>>::from_request_parts(&mut parts, &()).await
    {
        if let Some(deal) = params.get("deal_id") {
            if deal.trim().is_empty()
                || deal.len() > 64
                || !deal
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(AppError::bad_request("invalid dealId"));
            }
            use crate::domains::memberships::model::Capability;
            let path = parts.uri.path();
            let method = &parts.method;
            let capability = if path.contains("/data-room") {
                Capability::LocalSource
            } else if method == axum::http::Method::GET || path.contains("/documents/search/") {
                Capability::Read
            } else if path.ends_with("/documents/process")
                || path.ends_with("/documents/process_file")
            {
                Capability::Upload
            } else if path.ends_with("/archive") {
                Capability::Transfer
            } else if path.ends_with("/metadata")
                || path.ends_with("/extraction/upload")
                || path.ends_with("/update")
            {
                Capability::Edit
            } else if path.contains("/teams") {
                Capability::ManageTeams
            } else if path.ends_with("/ownership-transfer") {
                Capability::Transfer
            } else if path.contains("/members") {
                if method == axum::http::Method::DELETE {
                    if params.get("user_id").and_then(|id| id.parse::<i64>().ok())
                        == Some(actor.user.id)
                    {
                        Capability::Leave
                    } else {
                        Capability::RemoveMember
                    }
                } else {
                    Capability::ManageMembers
                }
            } else {
                return Err(AppError::Forbidden);
            }; // Canonical mutations repeat this policy in their write transaction.
            boundary
                .access
                .policy()
                .require(deal, actor.user.id, capability)
                .await?;
            stream_deal = Some(deal.clone());
        }
    }
    let is_read =
        parts.method == axum::http::Method::GET || parts.uri.path().contains("/documents/search/");
    let mut revoked = boundary.identity.revocations();
    parts.extensions.insert(actor.clone());
    let response = next.run(Request::from_parts(parts, body)).await;
    if is_read {
        if let Some(deal) = &stream_deal {
            boundary.access.deal(deal.clone(), actor.user.id).await?;
        }
    }
    if response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_none_or(|v| !v.as_bytes().starts_with(b"text/event-stream"))
    {
        return Ok(response);
    }
    let identity = boundary.identity;
    let access = boundary.access;
    let stopped = async move {
        let mut revalidate = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                event = revoked.recv() => {
                    match event {
                        Ok(hash) if hash != actor.token_hash => continue,
                        _ => break,
                    }
                }
                _ = revalidate.tick() => {
                    if identity.authenticate_hash(actor.token_hash.clone()).await.is_err() {
                        break;
                    }
                    if let Some(deal) = &stream_deal {
                        if access.deal(deal.clone(), actor.user.id).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    };
    let (parts, body) = response.into_parts();
    Ok(Response::from_parts(
        parts,
        Body::from_stream(body.into_data_stream().take_until(stopped)),
    ))
}
