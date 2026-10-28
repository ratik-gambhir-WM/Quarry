use super::model::RegisterInput;
use crate::{
    adapters::sqlite::{
        client::{SqliteClient, SqliteClientError},
        query::{ComparisonOperator, Condition, SqlBuilder},
    },
    domains::users::{User, UserRepository},
    shared::error::RepositoryError,
};

#[derive(Clone)]
pub struct IdentityRepository {
    sqlite: SqliteClient,
    users: UserRepository,
}
impl IdentityRepository {
    pub fn new(sqlite: SqliteClient, users: UserRepository) -> Self {
        Self { users, sqlite }
    }
    pub async fn credentials(
        &self,
        username: String,
    ) -> Result<Option<(i64, String)>, RepositoryError> {
        self.sqlite
            .read_one_with_async(
                SqlBuilder::select("users")
                    .columns(["id", "password_hash"])
                    .and_where(Condition::equal("username", username))
                    .build(),
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .await
            .map_err(Into::into)
    }
    pub async fn register(
        &self,
        input: RegisterInput,
        password_hash: String,
        token_hash: String,
        now: i64,
        expires: i64,
    ) -> Result<User, RepositoryError> {
        self.cleanup(now).await?;
        let id = self
            .sqlite
            .transaction_async(move |tx| {
                let inserted = tx.write(
                    SqlBuilder::insert_into("users")
                        .value("username", input.username)
                        .value("email", input.email)
                        .value("first_name", input.first_name)
                        .value("last_name", input.last_name)
                        .value("job_title", input.job_title)
                        .value("password_hash", password_hash)
                        .build(),
                )?;
                tx.write(session_insert(
                    token_hash,
                    inserted.last_insert_row_id,
                    now,
                    expires,
                ))?;
                Ok(inserted.last_insert_row_id)
            })
            .await
            .map_err(map_write_error)?;
        self.user(id).await
    }
    /// Bound maintenance work independently of authorization; expired rows are never accepted.
    async fn cleanup(&self, now: i64) -> Result<(), RepositoryError> {
        self.sqlite
            .transaction_async(move |tx| {
                let hashes = tx.read_with(
                    SqlBuilder::select("auth_sessions")
                        .columns(["token_hash"])
                        .and_where(Condition::compare(
                            "expires_at",
                            ComparisonOperator::LessThanOrEqual,
                            now,
                        ))
                        .limit(100)
                        .build(),
                    |row| row.get::<_, String>(0),
                )?;
                if !hashes.is_empty() {
                    tx.write(
                        SqlBuilder::delete_from("auth_sessions")
                            .and_where(Condition::in_values("token_hash", hashes))
                            .build(),
                    )?;
                }
                Ok(())
            })
            .await
            .map_err(map_write_error)
    }
    pub async fn user(&self, id: i64) -> Result<User, RepositoryError> {
        self.users
            .by_id(id)
            .await?
            .ok_or_else(|| RepositoryError::NotFound("user not found".into()))
    }
    pub async fn create_session(
        &self,
        id: i64,
        token_hash: String,
        now: i64,
        expires: i64,
    ) -> Result<(), RepositoryError> {
        self.cleanup(now).await?;
        self.sqlite
            .write_async(session_insert(token_hash, id, now, expires))
            .await
            .map_err(map_write_error)?;
        Ok(())
    }
    pub async fn session(&self, hash: String) -> Result<Option<(i64, i64)>, RepositoryError> {
        self.sqlite
            .read_one_with_async(
                SqlBuilder::select("auth_sessions")
                    .columns(["user_id", "expires_at"])
                    .and_where(Condition::equal("token_hash", hash))
                    .and_where(Condition::is_null("revoked_at"))
                    .build(),
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .await
            .map_err(Into::into)
    }
    pub async fn revoke(&self, hash: String, now: i64) -> Result<(), RepositoryError> {
        self.sqlite
            .write_async(
                SqlBuilder::update("auth_sessions")
                    .set("revoked_at", now)
                    .and_where(Condition::equal("token_hash", hash))
                    .build(),
            )
            .await
            .map_err(map_write_error)?;
        Ok(())
    }
}
fn session_insert(
    hash: String,
    id: i64,
    now: i64,
    expires: i64,
) -> Result<crate::adapters::sqlite::query::SqlQuery, SqliteClientError> {
    SqlBuilder::insert_into("auth_sessions")
        .value("token_hash", hash)
        .value("user_id", id)
        .value("created_at", now)
        .value("expires_at", expires)
        .build()
}
fn map_write_error(error: SqliteClientError) -> RepositoryError {
    if matches!(&error, SqliteClientError::Database(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation)
    {
        RepositoryError::Conflict("username or email is already registered".into())
    } else {
        RepositoryError::storage("identity storage operation failed")
    }
}
impl From<SqliteClientError> for RepositoryError {
    fn from(error: SqliteClientError) -> Self {
        Self::storage(error.to_string())
    }
}
