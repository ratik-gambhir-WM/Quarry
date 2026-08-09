use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::Value;

use crate::{
    core::{
        helix_queries::user::add_user::{
            add_user as build_add_user_query, create_user_indexes,
            get_user_by_email as build_get_user_by_email_query,
        },
        nodes::user_node::UserNode,
    },
    state::AppState,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    #[serde(serialize_with = "serialize_masked_api_key")]
    pub api_key: String,
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

fn serialize_masked_api_key<S>(api_key: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let masked = if api_key.len() <= 8 {
        "••••".to_string()
    } else {
        let prefix = api_key.chars().take(3).collect::<String>();
        let suffix = api_key
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        format!("{prefix}...{suffix}")
    };
    serializer.serialize_str(&masked)
}

pub struct CreateUserRecord<'a> {
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub email: &'a str,
    pub api_key: &'a str,
    pub role: &'a str,
}

pub fn create_user(state: &AppState, record: CreateUserRecord<'_>) -> Result<User, String> {
    state.gen_sqlite_db_client().execute(
        r#"
        INSERT INTO users (first_name, last_name, email, api_key, role)
        VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            record.first_name,
            record.last_name,
            record.email,
            record.api_key,
            record.role
        ],
    )?;

    get_user_by_email(state, record.email)?.ok_or_else(|| {
        format!(
            "failed to fetch user after insert for email `{}`",
            record.email
        )
    })
}

/// Persists a complete user row through the parallel Helix flow.
pub async fn upsert_wm_user(state: &AppState, user: UserNode) -> Result<Value, String> {
    let query = build_add_user_query(user)?;
    let helix = state.gen_helix_db_client();

    let _: Value = helix.execute_dynamic_query(create_user_indexes).await?;

    helix.execute_dynamic_query(move || query).await
}

/// Fetches one user from Helix by its exact email address.
pub async fn get_wm_user_by_email(state: &AppState, email: &str) -> Result<Value, String> {
    let query = build_get_user_by_email_query(email.to_string())?;

    state
        .gen_helix_db_client()
        .execute_dynamic_query(move || query)
        .await
}

pub fn get_user_by_email(state: &AppState, email: &str) -> Result<Option<User>, String> {
    state.with_sqlite_db(|connection| query_user_by_email(connection, email))
}

fn query_user_by_email(connection: &Connection, email: &str) -> rusqlite::Result<Option<User>> {
    connection
        .query_row(
            r#"
        SELECT id, first_name, last_name, email, api_key, role, created_at, updated_at
        FROM users
        WHERE email = ?1 COLLATE NOCASE
        ORDER BY id
        LIMIT 1
        "#,
            [email],
            user_from_row,
        )
        .optional()
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

#[cfg(test)]
#[path = "../../tests/repository/user_repository_tests.rs"]
mod tests;
