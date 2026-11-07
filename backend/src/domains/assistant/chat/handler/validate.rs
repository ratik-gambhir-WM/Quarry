use super::super::model::*;
use crate::app::http::error::{AppError, AppResult};

pub fn validate_prompt(value: String) -> AppResult<String> {
    validate_preserved_text("prompt", value, MAX_MESSAGE_CHARS)
}

pub fn validate_system_instructions(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|value| {
            validate_preserved_text("systemInstructions", value, MAX_SYSTEM_INSTRUCTIONS_CHARS)
        })
        .transpose()
}

pub fn validate_model(value: Option<String>) -> AppResult<Option<String>> {
    value
        .map(|value| {
            let model = value.trim();
            if model.is_empty()
                || model.chars().count() > MAX_MODEL_CHARS
                || model.chars().any(char::is_control)
            {
                return Err(AppError::bad_request("model is invalid"));
            }
            Ok(model.to_string())
        })
        .transpose()
}

pub fn parse_context(value: &str) -> AppResult<Vec<ChatContextMessage>> {
    let context: Vec<ChatContextMessage> = serde_json::from_str(value)
        .map_err(|_| AppError::bad_request("context must be a valid JSON message array"))?;
    if context.len() > MAX_CONTEXT_MESSAGES || !context.len().is_multiple_of(2) {
        return Err(AppError::bad_request(
            "context must contain at most 64 messages in complete user/assistant pairs",
        ));
    }
    let mut total_chars = 0usize;
    for (index, message) in context.iter().enumerate() {
        let expected_user = index % 2 == 0;
        if matches!(message.role, ChatContextRole::User) != expected_user {
            return Err(AppError::bad_request(
                "context roles must alternate user then assistant",
            ));
        }
        let count = message.content.chars().count();
        if message.content.trim().is_empty() || count > MAX_MESSAGE_CHARS {
            return Err(AppError::bad_request("context message content is invalid"));
        }
        total_chars = total_chars
            .checked_add(count)
            .ok_or_else(|| AppError::bad_request("context is too large"))?;
    }
    if total_chars > MAX_CONTEXT_CHARS {
        return Err(AppError::bad_request("context is too large"));
    }
    Ok(context)
}

fn validate_preserved_text(name: &str, value: String, max_chars: usize) -> AppResult<String> {
    if value.trim().is_empty() || value.chars().count() > max_chars {
        return Err(AppError::bad_request(format!("{name} is invalid")));
    }
    Ok(value)
}

pub(super) fn parse_file_references(
    value: Option<&str>,
    scope: Option<&str>,
    uploads: usize,
) -> AppResult<Vec<ChatFileReference>> {
    if let Some(scope) = scope {
        validate_identifier("fileScopeDealId", scope)?;
    }
    let references: Vec<ChatFileReference> = match value {
        Some(value) if value.len() <= 32_768 => serde_json::from_str(value)
            .map_err(|_| AppError::bad_request("fileReferences must be a valid JSON array"))?,
        Some(_) => return Err(AppError::bad_request("fileReferences is too large")),
        None => Vec::new(),
    };
    if references
        .len()
        .checked_add(uploads)
        .is_none_or(|count| count > MAX_CHAT_FILES)
    {
        return Err(AppError::bad_request(
            "at most 20 uploads and file references are allowed",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for reference in &references {
        validate_identifier("dealId", &reference.deal_id)?;
        validate_identifier("fileId", &reference.file_id)?;
        super::super::validate::validate_filename(&reference.name)
            .map_err(AppError::bad_request)?;
        if scope.is_some_and(|scope| scope != reference.deal_id) {
            return Err(AppError::bad_request(
                "referenced file is outside this data room",
            ));
        }
        if !seen.insert((&reference.deal_id, &reference.file_id)) {
            return Err(AppError::bad_request("duplicate file reference"));
        }
    }
    Ok(references)
}

pub(super) fn validate_identifier(name: &str, value: &str) -> AppResult<()> {
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

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/assistant/chat/handler/validate_tests.rs"]
mod tests;
pub(super) fn validate_user_email(email: &str) -> AppResult<()> {
    if email.trim().is_empty() || email.chars().count() > 320 {
        return Err(AppError::bad_request("userEmail is invalid"));
    }
    Ok(())
}

pub(super) fn validate_title(title: &str) -> AppResult<()> {
    if title.trim().is_empty() || title.chars().count() > 160 {
        return Err(AppError::bad_request("title is invalid"));
    }
    Ok(())
}

pub(super) fn validate_deal_id(id: &str) -> AppResult<()> {
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
