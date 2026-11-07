use super::super::{content, model::*};
use crate::app::http::error::{AppError, AppResult};
use std::collections::HashSet;

pub(super) fn uuid(value: &str) -> AppResult<()> {
    if uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value) {
        Ok(())
    } else {
        Err(AppError::bad_request("invalid resource id"))
    }
}

pub(super) fn deal(value: &str) -> AppResult<()> {
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

pub(super) fn label(value: String, max: usize, empty: bool) -> AppResult<String> {
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

pub(super) fn validate_save(mut input: SaveInput, patch: bool) -> AppResult<SaveInput> {
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

pub(super) fn list_options(query: Option<String>, kind: &str) -> AppResult<ListOptions> {
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
