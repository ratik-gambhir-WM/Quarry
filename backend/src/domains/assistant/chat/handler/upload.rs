use super::super::{
    model::{ChatFileReference, ChatUpload, PersistentQueryInput, RawUpload, MAX_CHAT_FILES},
    validate::{validate_filename, validate_upload},
};
use super::validate::{
    parse_file_references, validate_identifier, validate_model, validate_prompt,
    validate_system_instructions,
};
use crate::{
    app::http::error::{AppError, AppResult},
    shared::file_policy::{MAX_FILE_BYTES, MAX_TOTAL_REQUEST_FILE_BYTES},
};
use axum::extract::Multipart;

pub async fn collect_persistent_query_upload(
    mut multipart: Multipart,
    thread_id: String,
    user_email: String,
) -> AppResult<PersistentQueryInput> {
    let fields = collect_fields(&mut multipart).await?;
    validate_identifier("threadId", &thread_id)?;
    let user_message_id = required(fields.user_message_id, "userMessageId")?;
    let assistant_message_id = required(fields.assistant_message_id, "assistantMessageId")?;
    let request_id = required(fields.request_id, "requestId")?;
    validate_identifier("userMessageId", &user_message_id)?;
    validate_identifier("assistantMessageId", &assistant_message_id)?;
    validate_identifier("requestId", &request_id)?;
    if let Some(parent) = fields.parent_message_id.as_deref() {
        validate_identifier("parentMessageId", parent)?;
    }
    Ok(PersistentQueryInput {
        file_references: fields.file_references,
        file_scope_deal_id: fields.file_scope_deal_id,
        assistant_message_id,
        files: fields.files,
        model: validate_model(fields.model)?,
        parent_message_id: fields.parent_message_id,
        prompt: validate_prompt(required(fields.prompt, "prompt")?)?,
        request_id,
        system_instructions: validate_system_instructions(fields.system_instructions)?,
        thread_id,
        user_email: user_email.trim().to_string(),
        user_message_id,
    })
}

struct CollectedFields {
    file_references: Vec<ChatFileReference>,
    file_scope_deal_id: Option<String>,
    assistant_message_id: Option<String>,
    files: Vec<ChatUpload>,
    model: Option<String>,
    parent_message_id: Option<String>,
    prompt: Option<String>,
    request_id: Option<String>,
    system_instructions: Option<String>,
    user_message_id: Option<String>,
}

async fn collect_fields(multipart: &mut Multipart) -> AppResult<CollectedFields> {
    let mut file_references = None;
    let mut file_scope_deal_id = None;
    let mut prompt = None;
    let mut model = None;
    let mut system_instructions = None;
    let mut user_message_id = None;
    let mut assistant_message_id = None;
    let mut parent_message_id = None;
    let mut request_id = None;
    let mut raw_files = Vec::new();
    let mut total_bytes = 0usize;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("failed to read multipart field"))?
    {
        let name = field
            .name()
            .ok_or_else(|| AppError::bad_request("multipart field name is required"))?
            .to_string();
        match name.as_str() {
            "fileReferences" => {
                set_once(&mut file_references, field.text().await, "fileReferences")?
            }
            "fileScopeDealId" => set_once(
                &mut file_scope_deal_id,
                field.text().await,
                "fileScopeDealId",
            )?,
            "prompt" => set_once(&mut prompt, field.text().await, "prompt")?,
            "model" => set_once(&mut model, field.text().await, "model")?,
            "systemInstructions" => set_once(
                &mut system_instructions,
                field.text().await,
                "systemInstructions",
            )?,
            "userMessageId" => set_once(&mut user_message_id, field.text().await, "userMessageId")?,
            "assistantMessageId" => set_once(
                &mut assistant_message_id,
                field.text().await,
                "assistantMessageId",
            )?,
            "parentMessageId" => set_once(
                &mut parent_message_id,
                field.text().await,
                "parentMessageId",
            )?,
            "requestId" => set_once(&mut request_id, field.text().await, "requestId")?,
            "files" => {
                if raw_files.len() >= MAX_CHAT_FILES {
                    return Err(AppError::bad_request("at most 20 files are allowed"));
                }
                let filename = field
                    .file_name()
                    .ok_or_else(|| AppError::bad_request("file name is required"))?
                    .to_string();
                validate_filename(&filename).map_err(AppError::bad_request)?;
                let browser_mime = field.content_type().unwrap_or_default().to_string();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| AppError::bad_request("failed to read uploaded file"))?
                    .to_vec();
                if bytes.is_empty() || bytes.len() > MAX_FILE_BYTES {
                    return Err(AppError::bad_request("file is empty or exceeds 50 MB"));
                }
                total_bytes = total_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(|| AppError::bad_request("uploaded files are too large"))?;
                if total_bytes > MAX_TOTAL_REQUEST_FILE_BYTES {
                    return Err(AppError::bad_request(
                        "uploaded files exceed the 50 MB total request limit",
                    ));
                }
                raw_files.push(RawUpload {
                    browser_mime,
                    bytes,
                    filename,
                });
            }
            _ => return Err(AppError::bad_request("unknown multipart field")),
        }
    }

    let file_references = parse_file_references(
        file_references.as_deref(),
        file_scope_deal_id.as_deref(),
        raw_files.len(),
    )?;

    let files = tokio::task::spawn_blocking(move || {
        raw_files
            .into_iter()
            .map(|raw| validate_upload(raw).map_err(AppError::bad_request))
            .collect::<AppResult<Vec<_>>>()
    })
    .await
    .map_err(|error| AppError::internal(format!("file validation worker failed: {error}")))??;

    Ok(CollectedFields {
        file_references,
        file_scope_deal_id,
        assistant_message_id,
        files,
        model,
        parent_message_id,
        prompt,
        request_id,
        system_instructions,
        user_message_id,
    })
}

fn set_once(
    target: &mut Option<String>,
    value: Result<String, axum::extract::multipart::MultipartError>,
    name: &str,
) -> AppResult<()> {
    if target.is_some() {
        return Err(AppError::bad_request(format!("duplicate {name} field")));
    }
    *target = Some(value.map_err(|_| AppError::bad_request(format!("invalid {name} field")))?);
    Ok(())
}

fn required(value: Option<String>, name: &str) -> AppResult<String> {
    value.ok_or_else(|| AppError::bad_request(format!("{name} field is required")))
}
