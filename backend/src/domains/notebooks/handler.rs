use super::{content, model::*, service::NotebookService};
use crate::{
    app::http::error::{AppError, AppResult},
    domains::identity::model::AuthenticatedUser,
};
use axum::{
    extract::{FromRequest, FromRequestParts, Path, RawQuery, Request, State},
    http::{header, request::Parts, StatusCode},
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    Extension, Json,
};
use serde::de::DeserializeOwned;
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    sync::Arc,
    time::Duration,
};

#[derive(Clone)]
pub(super) struct NotebookHttpState {
    pub service: Arc<NotebookService>,
}
pub(super) struct Ids(HashMap<String, String>);
impl<S: Send + Sync> FromRequestParts<S> for Ids {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> AppResult<Self> {
        let Path(ids) = Path::<HashMap<String, String>>::from_request_parts(parts, state)
            .await
            .map_err(|_| AppError::bad_request("invalid resource path"))?;
        for id in ids.values() {
            uuid(id)?;
        }
        Ok(Self(ids))
    }
}
impl Ids {
    fn notebook(&self) -> String {
        self.0["notebook_id"].clone()
    }
    fn child(&self, name: &str) -> String {
        self.0[name].clone()
    }
}
pub(super) struct Input<T>(T);
impl<S, T> FromRequest<S> for Input<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AppError;
    async fn from_request(request: Request, state: &S) -> AppResult<Self> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(input)| Self(input))
            .map_err(|error| match error.status() {
                StatusCode::PAYLOAD_TOO_LARGE => AppError::PayloadTooLarge,
                StatusCode::UNSUPPORTED_MEDIA_TYPE => AppError::UnsupportedMediaType,
                _ => AppError::bad_request("invalid JSON request"),
            })
    }
}
fn uuid(value: &str) -> AppResult<()> {
    if uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value) {
        Ok(())
    } else {
        Err(AppError::bad_request("invalid resource id"))
    }
}
fn deal(value: &str) -> AppResult<()> {
    if value.starts_with("DEAL-")
        && value.len() > 5
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Ok(())
    } else {
        Err(AppError::bad_request("invalid dealId"))
    }
}
fn label(value: String, max: usize, empty: bool) -> AppResult<String> {
    if value.chars().any(char::is_control) {
        return Err(AppError::bad_request("invalid label"));
    }
    let value = value.trim();
    let size = value.chars().count();
    if size > max || (!empty && size == 0) {
        return Err(AppError::bad_request("invalid label length"));
    }
    Ok(value.to_owned())
}
fn validate_save(mut input: SaveInput, patch: bool) -> AppResult<SaveInput> {
    uuid(&input.operation_id)?;
    if patch {
        if !matches!(input.expected_revision, PatchField::Value(1..=MAX_REVISION)) {
            return Err(AppError::bad_request("expectedRevision is required"));
        }
        if input.title.missing()
            && input.folder_id.missing()
            && input.blocks.missing()
            && input.content_schema_version.missing()
        {
            return Err(AppError::bad_request("empty note patch"));
        }
        if input.blocks.missing() != input.content_schema_version.missing() {
            return Err(AppError::bad_request(
                "blocks and contentSchemaVersion must be supplied together",
            ));
        }
    } else if !input.expected_revision.missing() {
        return Err(AppError::bad_request("unexpected expectedRevision"));
    }
    input.title = match input.title {
        PatchField::Value(title) => PatchField::Value(label(title, 300, true)?),
        PatchField::Null => return Err(AppError::bad_request("title cannot be null")),
        value => value,
    };
    if let PatchField::Value(id) = &input.folder_id {
        uuid(id)?;
    }
    match &input.content_schema_version {
        PatchField::Missing | PatchField::Value(1) => {}
        _ => return Err(AppError::bad_request("unsupported contentSchemaVersion")),
    }
    match &input.blocks {
        PatchField::Null => return Err(AppError::bad_request("blocks cannot be null")),
        PatchField::Value(blocks) => content::validate(blocks).map_err(|error| {
            if error == "note content exceeds size limit" {
                AppError::PayloadTooLarge
            } else {
                AppError::bad_request(error)
            }
        })?,
        _ => {}
    }
    Ok(input)
}
fn list_options(query: Option<String>, kind: &str) -> AppResult<ListOptions> {
    let mut options = ListOptions {
        limit: 50,
        ..ListOptions::default()
    };
    let mut seen = HashSet::new();
    let query = query.unwrap_or_default();
    let bytes = query.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'%'
            && (i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit())
        {
            return Err(AppError::bad_request("invalid query encoding"));
        }
    }
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if !seen.insert(key.to_string()) {
            return Err(AppError::bad_request("repeated query parameter"));
        }
        match key.as_ref() {
            "limit" | "offset" => {
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(AppError::bad_request("invalid pagination"));
                }
                let number = value
                    .parse::<usize>()
                    .map_err(|_| AppError::bad_request("invalid pagination"))?;
                if key == "limit" {
                    if !(1..=100).contains(&number) {
                        return Err(AppError::bad_request("invalid limit"));
                    }
                    options.limit = number;
                } else {
                    if number > 100000 {
                        return Err(AppError::bad_request("invalid offset"));
                    }
                    options.offset = number;
                }
            }
            "dealId" if kind == "notebook" => {
                if value != "none" {
                    deal(&value)?;
                }
                options.relationship = Some(value.into_owned());
            }
            "folderId" if kind == "note" => {
                if value != "none" {
                    uuid(&value)?;
                }
                options.relationship = Some(value.into_owned());
            }
            "q" if kind == "note" => {
                options.query = Some(label(value.into_owned(), 200, false)?);
            }
            _ => return Err(AppError::bad_request("unknown query parameter")),
        }
    }
    Ok(options)
}
fn located<T: serde::Serialize>(status: StatusCode, location: String, value: T) -> Response {
    (status, [(header::LOCATION, location)], Json(value)).into_response()
}

pub(super) async fn create(
    State(s): State<NotebookHttpState>,
    Extension(actor): Extension<AuthenticatedUser>,
    Input(input): Input<NotebookInput>,
) -> AppResult<Response> {
    let title = match input.title {
        PatchField::Value(title) => label(title, 300, false)?,
        _ => return Err(AppError::bad_request("title is required")),
    };
    if let PatchField::Value(id) = &input.deal_id {
        deal(id)?;
    }
    let notebook = s
        .service
        .create(actor.user.id, title, input.deal_id.apply(None))
        .await?;
    Ok(located(
        StatusCode::CREATED,
        format!("/api/v1/notebooks/{}", notebook.notebook_id),
        notebook,
    ))
}
pub(super) async fn get(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<Json<Notebook>> {
    Ok(Json(
        s.service.get_notebook(a.user.id, ids.notebook()).await?,
    ))
}
pub(super) async fn list(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    RawQuery(query): RawQuery,
) -> AppResult<Json<Page<Notebooks>>> {
    Ok(Json(
        s.service
            .get_notebooks(a.user.id, list_options(query, "notebook")?)
            .await?,
    ))
}
pub(super) async fn update(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
    Input(input): Input<NotebookInput>,
) -> AppResult<Json<Notebook>> {
    if input.title.missing() && input.deal_id.missing() {
        return Err(AppError::bad_request("empty notebook patch"));
    }
    let title = match input.title {
        PatchField::Value(title) => Some(label(title, 300, false)?),
        PatchField::Missing => None,
        _ => return Err(AppError::bad_request("title cannot be null")),
    };
    if let PatchField::Value(id) = &input.deal_id {
        deal(id)?;
    }
    Ok(Json(
        s.service
            .update_notebook(a.user.id, ids.notebook(), title, input.deal_id)
            .await?,
    ))
}
pub(super) async fn delete(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<StatusCode> {
    s.service.delete_notebook(a.user.id, ids.notebook()).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn create_folder(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
    Input(input): Input<FolderInput>,
) -> AppResult<Response> {
    let f = s
        .service
        .create_folder(a.user.id, ids.notebook(), label(input.name, 200, false)?)
        .await?;
    Ok(located(
        StatusCode::CREATED,
        format!(
            "/api/v1/notebooks/{}/folders/{}",
            f.notebook_id, f.folder_id
        ),
        f,
    ))
}
pub(super) async fn get_folder(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<Json<Folder>> {
    Ok(Json(
        s.service
            .get_folder(a.user.id, ids.notebook(), ids.child("folder_id"))
            .await?,
    ))
}
pub(super) async fn list_folders(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
    RawQuery(query): RawQuery,
) -> AppResult<Json<Page<Folders>>> {
    Ok(Json(
        s.service
            .list_folders(a.user.id, ids.notebook(), list_options(query, "folder")?)
            .await?,
    ))
}
pub(super) async fn update_folder(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
    Input(input): Input<FolderInput>,
) -> AppResult<Json<Folder>> {
    Ok(Json(
        s.service
            .update_folder(
                a.user.id,
                ids.notebook(),
                ids.child("folder_id"),
                label(input.name, 200, false)?,
            )
            .await?,
    ))
}
pub(super) async fn delete_folder(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<StatusCode> {
    s.service
        .delete_folder(a.user.id, ids.notebook(), ids.child("folder_id"))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn create_note(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
    Input(input): Input<SaveInput>,
) -> AppResult<Response> {
    let job = s
        .service
        .save(
            a.user.id,
            ids.notebook(),
            None,
            validate_save(input, false)?,
        )
        .await?;
    Ok(located(StatusCode::ACCEPTED, job.status_url.clone(), job))
}
pub(super) async fn update_note(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
    Input(input): Input<SaveInput>,
) -> AppResult<Response> {
    let job = s
        .service
        .save(
            a.user.id,
            ids.notebook(),
            Some(ids.child("note_id")),
            validate_save(input, true)?,
        )
        .await?;
    Ok(located(StatusCode::ACCEPTED, job.status_url.clone(), job))
}
pub(super) async fn get_note(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<Json<Note>> {
    Ok(Json(
        s.service
            .get_note(a.user.id, ids.notebook(), ids.child("note_id"))
            .await?,
    ))
}
pub(super) async fn list_notes(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
    RawQuery(query): RawQuery,
) -> AppResult<Json<Page<Notes>>> {
    Ok(Json(
        s.service
            .list_notes(a.user.id, ids.notebook(), list_options(query, "note")?)
            .await?,
    ))
}
pub(super) async fn delete_note(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<StatusCode> {
    s.service
        .delete_note(a.user.id, ids.notebook(), ids.child("note_id"))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn job(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<Json<SaveJob>> {
    Ok(Json(
        s.service
            .job(a.user.id, ids.notebook(), ids.child("job_id"))
            .await?,
    ))
}
pub(super) async fn retry(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<Response> {
    let job = s
        .service
        .retry(a.user.id, ids.notebook(), ids.child("job_id"))
        .await?;
    Ok(located(StatusCode::ACCEPTED, job.status_url.clone(), job))
}
pub(super) async fn events(
    State(s): State<NotebookHttpState>,
    Extension(a): Extension<AuthenticatedUser>,
    ids: Ids,
) -> AppResult<Response> {
    let initial = s
        .service
        .job(a.user.id, ids.notebook(), ids.child("job_id"))
        .await?;
    let stream = futures_util::stream::unfold(
        (
            Some(initial),
            None,
            s.service,
            a.user.id,
            ids.notebook(),
            ids.child("job_id"),
            false,
        ),
        |(mut next, mut sequence, service, owner, notebook, id, finished)| async move {
            if finished {
                return None;
            }
            loop {
                let snapshot = if let Some(initial) = next.take() {
                    initial
                } else {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    match service.job(owner, notebook.clone(), id.clone()).await {
                        Ok(job) => job,
                        Err(_) => return None,
                    }
                };
                if sequence == Some(snapshot.sequence) {
                    continue;
                }
                sequence = Some(snapshot.sequence);
                let terminal = snapshot.terminal();
                let event = Event::default()
                    .event(&snapshot.status)
                    .id(format!(
                        "{}:{}:{}",
                        snapshot.job_id, snapshot.attempt, snapshot.sequence
                    ))
                    .json_data(&snapshot)
                    .ok()?;
                return Some((
                    Ok::<_, Infallible>(event),
                    (None, sequence, service, owner, notebook, id, terminal),
                ));
            }
        },
    );
    // Terminal snapshots end the stream on the next unfold without waiting for another update.
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response())
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/handler_tests.rs"]
mod tests;
