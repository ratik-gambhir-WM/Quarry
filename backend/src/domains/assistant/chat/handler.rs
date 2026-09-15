use std::{convert::Infallible, sync::Arc, time::Duration};

use axum::{
    extract::{Multipart, State},
    http::{header, HeaderMap, HeaderValue},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::{self, Stream};

use crate::app::http::error::{AppError, AppResult};

use super::{service::AssistantChatService, upload::collect_query_upload};

#[derive(Clone)]
pub(super) struct AssistantChatHttpState {
    pub assistant_chat: Arc<AssistantChatService>,
}

pub(super) async fn query_model_handler(
    State(state): State<AssistantChatHttpState>,
    multipart: Multipart,
) -> AppResult<(
    HeaderMap,
    Sse<impl Stream<Item = Result<Event, Infallible>>>,
)> {
    let input = collect_query_upload(multipart).await?;
    let receiver = state.assistant_chat.ask(input).map_err(AppError::from)?;
    let events = stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| {
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
    Ok((
        headers,
        Sse::new(events).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        ),
    ))
}
