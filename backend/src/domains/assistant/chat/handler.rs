use crate::domains::identity::model::AuthenticatedUser;
use axum::Extension;
use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::http::error::{AppError, AppResult};

use super::{
    service::{AssistantChatService, AssistantThread, AssistantThreadDetail, ThreadScope},
    upload::{collect_persistent_query_upload, collect_query_upload},
};

#[derive(Clone)]
pub(super) struct AssistantChatHttpState {
    pub assistant_chat: Arc<AssistantChatService>,
}

pub(super) async fn query_model_handler(
    Extension(_actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> AppResult<(
    HeaderMap,
    Sse<impl Stream<Item = Result<Event, Infallible>>>,
)> {
    let started_at = std::time::Instant::now();
    let http_request_id = request_id(&headers);
    tracing::info!(request_id = %http_request_id, route = "/query_model", "assistant request received");
    let input = match collect_query_upload(multipart).await {
        Ok(input) => input,
        Err(error) => {
            tracing::warn!(request_id = %http_request_id, error = %error, elapsed_ms = started_at.elapsed().as_millis(), "assistant request rejected while parsing multipart");
            return Err(error);
        }
    };
    tracing::info!(request_id = %http_request_id, context_messages = input.context.len(), file_count = input.files.len(), prompt_chars = input.prompt.chars().count(), "assistant request parsed");
    let receiver = match state.assistant_chat.ask(input) {
        Ok(receiver) => receiver,
        Err(error) => {
            tracing::warn!(request_id = %http_request_id, error = %error, elapsed_ms = started_at.elapsed().as_millis(), "assistant request rejected while starting run");
            return Err(AppError::from(error));
        }
    };
    tracing::info!(request_id = %http_request_id, elapsed_ms = started_at.elapsed().as_millis(), "assistant stream opened");
    Ok(query_sse(receiver))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ThreadListQuery {
    #[serde(skip_deserializing)]
    user_email: String,
    archived: Option<bool>,
    limit: Option<usize>,
    before: Option<String>,
    scope: Option<String>,
    deal_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OwnerQuery {
    #[serde(skip_deserializing)]
    user_email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CreateThreadInput {
    #[serde(skip_deserializing)]
    user_email: String,
    thread_id: Option<String>,
    deal_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ThreadMutationInput {
    #[serde(skip_deserializing)]
    user_email: String,
    title: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ThreadListResponse {
    threads: Vec<AssistantThread>,
    next_cursor: Option<String>,
}

pub(super) async fn list_threads_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    Query(mut query): Query<ThreadListQuery>,
) -> AppResult<Json<ThreadListResponse>> {
    query.user_email = actor.user.email;
    validate_user_email(&query.user_email)?;
    let limit = query.limit.unwrap_or(30);
    if !(1..=100).contains(&limit) {
        return Err(AppError::bad_request("invalid thread page size"));
    }
    let scope = match (query.scope.as_deref().unwrap_or("all"), query.deal_id) {
        ("all", None) => ThreadScope::All,
        ("freeform", None) => ThreadScope::Freeform,
        ("deal", Some(id)) => {
            validate_deal_id(&id)?;
            ThreadScope::Deal(id)
        }
        _ => {
            return Err(AppError::bad_request(
                "invalid thread scope/dealId combination",
            ))
        }
    };
    let threads = state
        .assistant_chat
        .list_threads(
            &query.user_email,
            query.archived.unwrap_or(false),
            limit + 1,
            query.before,
            scope,
        )
        .await
        .map_err(AppError::from)?;
    let next_cursor = (threads.len() > limit).then(|| {
        let thread = &threads[limit - 1];
        format!("{}|{}", thread.last_message_at, thread.thread_id)
    });
    Ok(Json(ThreadListResponse {
        threads: threads.into_iter().take(limit).collect(),
        next_cursor,
    }))
}

pub(super) async fn create_thread_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    Json(mut input): Json<CreateThreadInput>,
) -> AppResult<(StatusCode, Json<AssistantThread>)> {
    input.user_email = actor.user.email;
    validate_user_email(&input.user_email)?;
    let thread_id = input
        .thread_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    validate_identifier("threadId", &thread_id)?;
    if let Some(id) = &input.deal_id {
        validate_deal_id(id)?;
    }
    let thread = state
        .assistant_chat
        .create_thread(&input.user_email, thread_id, input.deal_id)
        .await
        .map_err(AppError::from)?;
    Ok((StatusCode::CREATED, Json(thread)))
}

pub(super) async fn get_thread_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    Path(thread_id): Path<String>,
    Query(mut query): Query<OwnerQuery>,
) -> AppResult<Json<AssistantThreadDetail>> {
    query.user_email = actor.user.email;
    validate_user_email(&query.user_email)?;
    validate_identifier("threadId", &thread_id)?;
    state
        .assistant_chat
        .get_thread(&query.user_email, thread_id)
        .await
        .map(Json)
        .map_err(AppError::from)
}

pub(super) async fn rename_thread_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    Path(thread_id): Path<String>,
    Json(mut input): Json<ThreadMutationInput>,
) -> AppResult<StatusCode> {
    let title = input
        .title
        .ok_or_else(|| AppError::bad_request("title is required"))?;
    input.user_email = actor.user.email;
    validate_user_email(&input.user_email)?;
    validate_identifier("threadId", &thread_id)?;
    validate_title(&title)?;
    state
        .assistant_chat
        .rename_thread(&input.user_email, thread_id, title)
        .await
        .map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn archive_thread_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    Path(thread_id): Path<String>,
    Json(mut input): Json<ThreadMutationInput>,
) -> AppResult<StatusCode> {
    input.user_email = actor.user.email;
    validate_user_email(&input.user_email)?;
    validate_identifier("threadId", &thread_id)?;
    state
        .assistant_chat
        .set_thread_archived(&input.user_email, thread_id, true)
        .await
        .map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn unarchive_thread_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    Path(thread_id): Path<String>,
    Json(mut input): Json<ThreadMutationInput>,
) -> AppResult<StatusCode> {
    input.user_email = actor.user.email;
    validate_user_email(&input.user_email)?;
    validate_identifier("threadId", &thread_id)?;
    state
        .assistant_chat
        .set_thread_archived(&input.user_email, thread_id, false)
        .await
        .map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn delete_thread_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    Path(thread_id): Path<String>,
    Query(mut query): Query<OwnerQuery>,
) -> AppResult<StatusCode> {
    query.user_email = actor.user.email;
    validate_user_email(&query.user_email)?;
    validate_identifier("threadId", &thread_id)?;
    state
        .assistant_chat
        .delete_thread(&query.user_email, thread_id)
        .await
        .map_err(AppError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn run_thread_handler(
    Extension(actor): Extension<AuthenticatedUser>,
    State(state): State<AssistantChatHttpState>,
    headers: HeaderMap,
    Path(thread_id): Path<String>,
    multipart: Multipart,
) -> AppResult<(
    HeaderMap,
    Sse<impl Stream<Item = Result<Event, Infallible>>>,
)> {
    let started_at = std::time::Instant::now();
    let http_request_id = request_id(&headers);
    tracing::info!(request_id = %http_request_id, thread_id = %thread_id, route = "/assistant/threads/{thread_id}/runs", "assistant persisted request received");
    let input = match collect_persistent_query_upload(multipart, thread_id, actor.user.email).await
    {
        Ok(input) => input,
        Err(error) => {
            tracing::warn!(request_id = %http_request_id, error = %error, elapsed_ms = started_at.elapsed().as_millis(), "assistant persisted request rejected while parsing multipart");
            return Err(error);
        }
    };
    tracing::info!(request_id = %http_request_id, thread_id = %input.thread_id, client_request_id = %input.request_id, file_count = input.files.len(), prompt_chars = input.prompt.chars().count(), "assistant persisted request parsed");
    let receiver = match state.assistant_chat.ask_persisted(input).await {
        Ok(receiver) => receiver,
        Err(error) => {
            tracing::warn!(request_id = %http_request_id, error = %error, elapsed_ms = started_at.elapsed().as_millis(), "assistant persisted request rejected while starting run");
            return Err(AppError::from(error));
        }
    };
    tracing::info!(request_id = %http_request_id, elapsed_ms = started_at.elapsed().as_millis(), "assistant persisted stream opened");
    Ok(query_sse(receiver))
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("missing")
        .to_string()
}

fn validate_user_email(email: &str) -> AppResult<()> {
    if email.trim().is_empty() || email.chars().count() > 320 {
        return Err(AppError::bad_request("userEmail is invalid"));
    }
    Ok(())
}

fn validate_identifier(name: &str, value: &str) -> AppResult<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::bad_request(format!("{name} is invalid")));
    }
    Ok(())
}

fn validate_title(title: &str) -> AppResult<()> {
    if title.trim().is_empty() || title.chars().count() > 160 {
        return Err(AppError::bad_request("title is invalid"));
    }
    Ok(())
}

fn query_sse(
    receiver: tokio::sync::mpsc::Receiver<super::model::SendQueryEvent>,
) -> (
    HeaderMap,
    Sse<impl Stream<Item = Result<Event, Infallible>>>,
) {
    let events = stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| {
            tracing::debug!(event = event.event_name(), "assistant SSE event emitted");
            let encoded = Event::default()
                .event(event.event_name())
                .json_data(&event)
                .unwrap_or_else(|_| {
                    Event::default()
                        .event("failed")
                        .data(r#"{"type":"failed","error":"query generation failed"}"#)
                });
            (Ok(encoded), receiver)
        })
    });
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-store, no-transform"),
    );
    headers.insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_static("identity"),
    );
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    (
        headers,
        Sse::new(events).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        ),
    )
}

fn validate_deal_id(id: &str) -> AppResult<()> {
    if !id.starts_with("DEAL-")
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(AppError::bad_request("invalid dealId"));
    }
    Ok(())
}
