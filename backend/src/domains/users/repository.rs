use rusqlite::Row;
use serde::Serialize;

use crate::{
    adapters::sqlite::{
        client::SqliteClient,
        query::{Condition, SqlBuilder},
    },
    shared::error::RepositoryError,
};

pub(crate) const USER_COLUMNS: [&str; 8] = [
    "id",
    "first_name",
    "last_name",
    "email",
    "username",
    "job_title",
    "created_at",
    "updated_at",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub username: String,
    pub job_title: Option<String>,
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

    pub async fn by_email(&self, email: String) -> Result<Option<User>, RepositoryError> {
        let query = SqlBuilder::select("users")
            .columns(USER_COLUMNS)
            .and_where(Condition::equal("email", email))
            .build();
        self.sqlite
            .read_one_with_async(query, user_from_row)
            .await
            .map_err(|error| RepositoryError::storage(format!("failed to read user: {error}")))
    }

    pub(crate) async fn by_id(&self, id: i64) -> Result<Option<User>, RepositoryError> {
        let query = SqlBuilder::select("users")
            .columns(USER_COLUMNS)
            .and_where(Condition::equal("id", id))
            .build();
        self.sqlite
            .read_one_with_async(query, user_from_row)
            .await
            .map_err(|error| RepositoryError::storage(format!("failed to read user: {error}")))
    }
}

pub(crate) fn user_from_row(row: &Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get("id")?,
        first_name: row.get("first_name")?,
        last_name: row.get("last_name")?,
        email: row.get("email")?,
        username: row.get("username")?,
        job_title: row.get("job_title")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
#[path = "../../../tests/integration/domains/users/repository_support.rs"]
pub(crate) mod test_support;
