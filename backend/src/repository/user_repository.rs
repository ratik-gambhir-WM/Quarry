use rusqlite::Row;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        clients::sqlite::SqliteClient,
        sqlbuilder::{Condition, SqlBuilder, SqlBuilderError, SqlQuery},
    },
    repository::RepositoryError,
};

const USER_COLUMNS: [&str; 8] = [
    "id",
    "first_name",
    "last_name",
    "email",
    "api_key",
    "role",
    "created_at",
    "updated_at",
];

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

#[derive(Clone)]
pub struct UserRepository {
    sqlite: SqliteClient,
}

impl UserRepository {
    pub fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }

    pub async fn create(&self, input: AddUserInput) -> Result<User, RepositoryError> {
        let insert = build_query(
            SqlBuilder::insert_into("users")
                .value("first_name", input.first_name.trim())
                .value("last_name", input.last_name.trim())
                .value("email", input.email.trim())
                .value("api_key", input.api_key.trim())
                .value("role", input.role.trim())
                .build(),
            "user insert",
        )?;
        let inserted =
            self.sqlite.write_async(insert).await.map_err(|error| {
                RepositoryError::storage(format!("failed to create user: {error}"))
            })?;

        self.by_id(inserted.last_insert_row_id)
            .await?
            .ok_or_else(|| {
                RepositoryError::storage(format!(
                    "inserted user `{}` could not be loaded",
                    inserted.last_insert_row_id
                ))
            })
    }

    pub async fn by_email(&self, email: String) -> Result<Option<User>, RepositoryError> {
        let query = build_query(
            SqlBuilder::select("users")
                .columns(USER_COLUMNS)
                .and_where(Condition::equal("email", email))
                .build(),
            "user email lookup",
        )?;
        self.sqlite
            .read_one_with_async(query, user_from_row)
            .await
            .map_err(|error| RepositoryError::storage(format!("failed to read user: {error}")))
    }

    async fn by_id(&self, id: i64) -> Result<Option<User>, RepositoryError> {
        let query = build_query(
            SqlBuilder::select("users")
                .columns(USER_COLUMNS)
                .and_where(Condition::equal("id", id))
                .build(),
            "user id lookup",
        )?;
        self.sqlite
            .read_one_with_async(query, user_from_row)
            .await
            .map_err(|error| RepositoryError::storage(format!("failed to read user: {error}")))
    }
}

fn build_query(
    query: Result<SqlQuery, SqlBuilderError>,
    operation: &str,
) -> Result<SqlQuery, RepositoryError> {
    query.map_err(|error| {
        RepositoryError::storage(format!("failed to build {operation} query: {error}"))
    })
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
