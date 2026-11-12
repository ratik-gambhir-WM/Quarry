use super::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserInput {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub role: String,
}

impl UserRepository {
    pub async fn create(&self, input: AddUserInput) -> Result<User, RepositoryError> {
        let insert = SqlBuilder::insert_into("users")
            .value("first_name", input.first_name.trim())
            .value("last_name", input.last_name.trim())
            .value("email", input.email.trim())
            .value("username", input.email.trim().to_ascii_lowercase())
            .value("password_hash", "test-only-unusable-password")
            .value("job_title", input.role.trim())
            .build();
        let inserted =
            self.postgres.write_async(insert).await.map_err(|error| {
                RepositoryError::storage(format!("failed to create user: {error}"))
            })?;

        self.by_id(inserted.inserted_id).await?.ok_or_else(|| {
            RepositoryError::storage(format!(
                "inserted user `{}` could not be loaded",
                inserted.inserted_id
            ))
        })
    }
}
