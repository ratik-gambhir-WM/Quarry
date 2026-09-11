pub(crate) mod handler;
pub(crate) mod repository;
pub(crate) mod route;
pub(crate) mod service;

use repository::UserRepository;
use shared::UserDirectoryResult;

pub use repository::User;

mod shared {
    pub type UserDirectoryResult<T> = Result<T, crate::shared::error::ServiceError>;
}

#[derive(Clone)]
pub struct UserDirectory {
    repository: UserRepository,
}

impl UserDirectory {
    pub(crate) fn new(repository: UserRepository) -> Self {
        Self { repository }
    }

    pub async fn by_email(&self, email: &str) -> UserDirectoryResult<Option<User>> {
        self.repository
            .by_email(email.to_string())
            .await
            .map_err(Into::into)
    }
}
