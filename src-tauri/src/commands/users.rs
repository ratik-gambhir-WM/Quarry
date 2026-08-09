use serde_json::Value;
use tauri::State;

use crate::{
    commands::{CommandResult, CommandResultExt},
    core::nodes::user_node::UserNode,
    services::user_service::{
        add_user, add_wm_user, get_user_by_email as fetch_user_by_email,
        get_wm_user_by_email as fetch_wm_user_by_email, AddUserInput, User,
    },
    state::AppState,
};

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub fn create_user(state: State<'_, AppState>, input: AddUserInput) -> CommandResult<User> {
    validate_user_input(&input).validation_context("create_user")?;
    add_user(&state, input).command_context("create_user")
}

#[tauri::command]
pub fn user_exists_by_email(state: State<'_, AppState>, email: String) -> CommandResult<bool> {
    validate_email(&email).validation_context("user_exists_by_email")?;
    fetch_user_by_email(&state, &email)
        .map(|user| user.is_some())
        .command_context("user_exists_by_email")
}

#[tauri::command]
pub fn get_user_by_email(state: State<'_, AppState>, email: String) -> CommandResult<Option<User>> {
    validate_email(&email).validation_context("get_user_by_email")?;
    fetch_user_by_email(&state, &email).command_context("get_user_by_email")
}

#[tauri::command]
pub async fn create_wm_user(state: State<'_, AppState>, input: UserNode) -> CommandResult<Value> {
    validate_wm_user_input(&input).validation_context("create_wm_user")?;

    add_wm_user(&state, input)
        .await
        .map(redact_user_secrets)
        .command_context("create_wm_user")
}

#[tauri::command]
pub async fn get_wm_user_by_email(
    state: State<'_, AppState>,
    email: String,
) -> CommandResult<Value> {
    validate_email(&email).validation_context("get_wm_user_by_email")?;

    fetch_wm_user_by_email(&state, &email)
        .await
        .map(redact_user_secrets)
        .command_context("get_wm_user_by_email")
}

fn redact_user_secrets(mut value: Value) -> Value {
    fn visit(value: &mut Value) {
        match value {
            Value::Object(object) => {
                for (key, nested) in object {
                    if matches!(
                        key.as_str(),
                        "api_key" | "apiKey" | "authorization" | "token"
                    ) {
                        *nested = Value::String("[REDACTED]".to_string());
                    } else {
                        visit(nested);
                    }
                }
            }
            Value::Array(values) => values.iter_mut().for_each(visit),
            _ => {}
        }
    }
    visit(&mut value);
    value
}

fn validate_email(email: &str) -> Result<(), String> {
    if email.trim().is_empty() {
        return Err("email is required".to_string());
    }

    Ok(())
}

fn validate_user_input(input: &AddUserInput) -> Result<(), String> {
    if input.first_name.trim().is_empty() {
        return Err("first_name is required".to_string());
    }

    if input.last_name.trim().is_empty() {
        return Err("last_name is required".to_string());
    }

    validate_email(&input.email)?;

    if input.api_key.trim().is_empty() {
        return Err("api_key is required".to_string());
    }

    if input.role.trim().is_empty() {
        return Err("role is required".to_string());
    }

    Ok(())
}

fn validate_wm_user_input(input: &UserNode) -> Result<(), String> {
    if input.id <= 0 {
        return Err("id is required".to_string());
    }

    if input.first_name.trim().is_empty() {
        return Err("first_name is required".to_string());
    }

    if input.last_name.trim().is_empty() {
        return Err("last_name is required".to_string());
    }

    validate_email(&input.email)?;

    if input.api_key.trim().is_empty() {
        return Err("api_key is required".to_string());
    }

    if input.role.trim().is_empty() {
        return Err("role is required".to_string());
    }

    if input.created_at.trim().is_empty() {
        return Err("created_at is required".to_string());
    }

    if input.updated_at.trim().is_empty() {
        return Err("updated_at is required".to_string());
    }

    Ok(())
}

#[cfg(test)]
#[path = "../../tests/commands/users_tests.rs"]
mod tests;
