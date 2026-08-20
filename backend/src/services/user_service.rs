use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    core::{
        helix_queries::user::persistence::{
            create_user_indexes, get_user_by_email as build_get_user_by_email_query,
            save_user as build_save_user_query,
        },
        models::user::UserNode,
    },
    state::AppState,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserInput {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub api_key: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub api_key: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn save_sqlite_user(state: &AppState, input: AddUserInput) -> Result<User, String> {
    validate_user_input(&input)?;

    state.with_db(|db| {
        db.execute(
            r#"
            INSERT INTO users (first_name, last_name, email, api_key, role)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                input.first_name.trim(),
                input.last_name.trim(),
                input.email.trim(),
                input.api_key.trim(),
                input.role.trim()
            ],
        )?;

        get_user_by_id(db, db.last_insert_rowid())
    })
}

pub fn get_sqlite_user_by_email(state: &AppState, email: &str) -> Result<Option<User>, String> {
    let email = email.trim();
    if email.is_empty() {
        return Err("email is required".to_string());
    }

    state.with_db(|db| {
        db.query_row(
            r#"
            SELECT id, first_name, last_name, email, api_key, role, created_at, updated_at
            FROM users
            WHERE email = ?1
            "#,
            [email],
            user_from_row,
        )
        .optional()
    })
}

pub async fn save_helix_user(state: &AppState, input: UserNode) -> Result<Value, String> {
    validate_helix_user_input(&input)?;
    let query = build_save_user_query(input)?;
    let _: Value = state
        .helix()
        .execute_dynamic_query(create_user_indexes)
        .await?;
    state.helix().execute_dynamic_query(move || query).await
}

pub async fn get_helix_user_by_email(state: &AppState, email: &str) -> Result<Value, String> {
    let email = email.trim();
    if email.is_empty() {
        return Err("email is required".to_string());
    }
    let query = build_get_user_by_email_query(email.to_string())?;
    state.helix().execute_dynamic_query(move || query).await
}

fn get_user_by_id(db: &Connection, id: i64) -> rusqlite::Result<User> {
    db.query_row(
        r#"
        SELECT id, first_name, last_name, email, api_key, role, created_at, updated_at
        FROM users
        WHERE id = ?1
        "#,
        [id],
        user_from_row,
    )
}

fn user_from_row(row: &Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get("id")?,
        first_name: row.get("first_name")?,
        last_name: row.get("last_name")?,
        email: row.get("email")?,
        api_key: row.get("api_key")?,
        role: row.get("role")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn validate_user_input(input: &AddUserInput) -> Result<(), String> {
    if input.first_name.trim().is_empty() {
        return Err("first_name is required".to_string());
    }

    if input.last_name.trim().is_empty() {
        return Err("last_name is required".to_string());
    }

    if input.email.trim().is_empty() {
        return Err("email is required".to_string());
    }

    if input.api_key.trim().is_empty() {
        return Err("api_key is required".to_string());
    }

    if input.role.trim().is_empty() {
        return Err("role is required".to_string());
    }

    Ok(())
}

fn validate_helix_user_input(input: &UserNode) -> Result<(), String> {
    if input.id <= 0 {
        return Err("id is required".to_string());
    }
    if input.first_name.trim().is_empty() {
        return Err("first_name is required".to_string());
    }
    if input.last_name.trim().is_empty() {
        return Err("last_name is required".to_string());
    }
    if input.email.trim().is_empty() {
        return Err("email is required".to_string());
    }
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
